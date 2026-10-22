//! Streams cpal: entrada (mic), saída principal (BlackHole) e monitor opcional.

use std::sync::atomic::Ordering::Relaxed;
use std::sync::Arc;

use anyhow::{anyhow, Result};
use cpal::traits::{DeviceTrait, StreamTrait};
use cpal::{
    Device, FromSample, Sample, SampleFormat, SampleRate, SizedSample, Stream, StreamConfig,
    SupportedStreamConfig, SupportedStreamConfigRange,
};
use crossbeam_channel::{bounded, Receiver, Sender};
use ringbuf::traits::{Consumer, Observer, Producer, Split};
use ringbuf::{HeapCons, HeapProd, HeapRb};

use super::mixer::{Command, Mixer};
use super::resample::LinearResampler;
use super::shared::Shared;
use crate::devices;

const PREFERRED_RATE: u32 = 48_000;

pub struct AudioEngine {
    streams: Vec<Stream>,
    cmd_tx: Sender<Command>,
    pub sample_rate: u32,
    pub output_name: Option<String>,
    pub input_name: Option<String>,
    pub errors: Vec<String>,
}

impl AudioEngine {
    pub fn start(input: Option<&str>, output: Option<&str>, monitor: Option<&str>, shared: Arc<Shared>) -> Self {
        let host = cpal::default_host();
        let (cmd_tx, cmd_rx) = bounded(256);
        let mut engine = Self {
            streams: Vec::new(),
            cmd_tx,
            sample_rate: PREFERRED_RATE,
            output_name: None,
            input_name: None,
            errors: Vec::new(),
        };
        shared.stream_error.store(false, Relaxed);
        shared.reset_progress();

        let Some(out_dev) = devices::find_output(&host, output) else {
            engine.errors.push(format!("Saída não encontrada: {}", output.unwrap_or("padrão")));
            return engine;
        };
        let out_cfg = match pick_config(out_dev.supported_output_configs().map(|c| c.collect()), out_dev.default_output_config(), PREFERRED_RATE, 2) {
            Ok(c) => c,
            Err(e) => {
                engine.errors.push(format!("Saída: {e}"));
                return engine;
            }
        };
        let rate = out_cfg.sample_rate().0;
        engine.sample_rate = rate;
        engine.output_name = out_dev.name().ok();

        let (mic_prod, mic_cons) = HeapRb::<f32>::new(rate as usize).split();

        // Monitor (fones): recebe só o soundboard já mixado pelo callback principal.
        let mut monitor_prod = None;
        if let Some(name) = monitor {
            match open_monitor(&host, name, rate, shared.clone()) {
                Ok((stream, prod)) => {
                    engine.streams.push(stream);
                    monitor_prod = Some(prod);
                }
                Err(e) => engine.errors.push(format!("Monitor: {e}")),
            }
        }

        // Microfone salvo desconectado: cai no padrão do sistema em vez de ficar sem áudio.
        let input = match input {
            Some(n) if devices::find_input(&host, Some(n)).is_none() => {
                engine.errors.push(format!("Microfone \"{n}\" não encontrado; usando o padrão do sistema."));
                None
            }
            other => other,
        };
        match open_input(&host, input, rate, mic_prod, shared.clone()) {
            Ok((stream, name)) => {
                engine.streams.push(stream);
                engine.input_name = Some(name);
            }
            Err(e) => engine.errors.push(format!(
                "Microfone: {e}. Verifique Ajustes do Sistema > Privacidade e Segurança > Microfone."
            )),
        }

        let state = OutputState {
            mixer: Mixer::new(),
            rx: cmd_rx,
            mic_cons,
            monitor_prod,
            shared: shared.clone(),
            mic_buf: vec![0.0; 4096],
            out_buf: vec![0.0; 8192],
            mon_buf: vec![0.0; 8192],
            max_backlog: rate as usize / 25,
            target_backlog: rate as usize / 200,
        };
        let config: StreamConfig = out_cfg.config();
        let channels = config.channels as usize;
        let err_shared = shared.clone();
        let result = match out_cfg.sample_format() {
            SampleFormat::F32 => build_output::<f32>(&out_dev, &config, state, channels, err_shared),
            SampleFormat::I16 => build_output::<i16>(&out_dev, &config, state, channels, err_shared),
            SampleFormat::U16 => build_output::<u16>(&out_dev, &config, state, channels, err_shared),
            SampleFormat::I32 => build_output::<i32>(&out_dev, &config, state, channels, err_shared),
            f => Err(anyhow!("formato de amostra não suportado: {f}")),
        };
        match result {
            Ok(s) => engine.streams.push(s),
            Err(e) => engine.errors.push(format!("Saída: {e}")),
        }

        for s in &engine.streams {
            if let Err(e) = s.play() {
                engine.errors.push(format!("Falha ao iniciar stream: {e}"));
            }
        }
        engine
    }

    pub fn stop(&mut self) {
        self.streams.clear();
    }

    pub fn send(&self, cmd: Command) {
        let _ = self.cmd_tx.try_send(cmd);
    }
}

fn err_fn(shared: Arc<Shared>) -> impl FnMut(cpal::StreamError) + Send + 'static {
    move |e| {
        log::error!("erro no stream de áudio: {e}");
        shared.stream_error.store(true, Relaxed);
    }
}

/// Prefere a taxa pedida, f32 e o menor número de canais >= `min_channels`.
fn pick_config(
    ranges: Result<Vec<SupportedStreamConfigRange>, cpal::SupportedStreamConfigsError>,
    default: Result<SupportedStreamConfig, cpal::DefaultStreamConfigError>,
    rate: u32,
    min_channels: u16,
) -> Result<SupportedStreamConfig> {
    if let Ok(ranges) = ranges {
        let mut candidates: Vec<_> = ranges
            .into_iter()
            .filter(|r| r.min_sample_rate().0 <= rate && r.max_sample_rate().0 >= rate)
            .collect();
        candidates.sort_by_key(|r| (r.sample_format() != SampleFormat::F32, r.channels() < min_channels, r.channels()));
        if let Some(r) = candidates.into_iter().next() {
            return Ok(r.with_sample_rate(SampleRate(rate)));
        }
    }
    Ok(default?)
}

fn open_input(
    host: &cpal::Host,
    name: Option<&str>,
    rate: u32,
    prod: HeapProd<f32>,
    shared: Arc<Shared>,
) -> Result<(Stream, String)> {
    let dev = devices::find_input(host, name).ok_or_else(|| anyhow!("dispositivo não encontrado"))?;
    let cfg = pick_config(dev.supported_input_configs().map(|c| c.collect()), dev.default_input_config(), rate, 1)?;
    let config: StreamConfig = cfg.config();
    let state = InputState {
        prod,
        channels: config.channels as usize,
        resampler: LinearResampler::new(cfg.sample_rate().0, rate),
    };
    let stream = match cfg.sample_format() {
        SampleFormat::F32 => build_input::<f32>(&dev, &config, state, shared),
        SampleFormat::I16 => build_input::<i16>(&dev, &config, state, shared),
        SampleFormat::U16 => build_input::<u16>(&dev, &config, state, shared),
        SampleFormat::I32 => build_input::<i32>(&dev, &config, state, shared),
        f => Err(anyhow!("formato de amostra não suportado: {f}")),
    }?;
    Ok((stream, dev.name().unwrap_or_default()))
}

fn open_monitor(host: &cpal::Host, name: &str, rate: u32, shared: Arc<Shared>) -> Result<(Stream, HeapProd<f32>)> {
    let dev = devices::find_output(host, Some(name)).ok_or_else(|| anyhow!("dispositivo não encontrado"))?;
    let cfg = pick_config(dev.supported_output_configs().map(|c| c.collect()), dev.default_output_config(), rate, 2)?;
    if cfg.sample_rate().0 != rate {
        return Err(anyhow!("o dispositivo não suporta {rate} Hz"));
    }
    let (prod, cons) = HeapRb::<f32>::new(rate as usize * 2).split();
    let config: StreamConfig = cfg.config();
    let state = MonitorState {
        cons,
        buf: vec![0.0; 8192],
        max_backlog: rate as usize / 25 * 2,
        target_backlog: rate as usize / 200 * 2,
    };
    let channels = config.channels as usize;
    let stream = match cfg.sample_format() {
        SampleFormat::F32 => build_monitor::<f32>(&dev, &config, state, channels, shared),
        SampleFormat::I16 => build_monitor::<i16>(&dev, &config, state, channels, shared),
        SampleFormat::U16 => build_monitor::<u16>(&dev, &config, state, channels, shared),
        SampleFormat::I32 => build_monitor::<i32>(&dev, &config, state, channels, shared),
        f => Err(anyhow!("formato de amostra não suportado: {f}")),
    }?;
    Ok((stream, prod))
}

struct InputState {
    prod: HeapProd<f32>,
    channels: usize,
    resampler: LinearResampler,
}

fn build_input<T>(dev: &Device, config: &StreamConfig, mut st: InputState, shared: Arc<Shared>) -> Result<Stream>
where
    T: SizedSample,
    f32: FromSample<T>,
{
    let stream = dev.build_input_stream(
        config,
        move |data: &[T], _: &cpal::InputCallbackInfo| {
            let InputState { prod, channels, resampler } = &mut st;
            for frame in data.chunks_exact(*channels) {
                let sum: f32 = frame.iter().map(|&s| f32::from_sample(s)).sum();
                resampler.push(sum / *channels as f32, |x| {
                    let _ = prod.try_push(x);
                });
            }
        },
        err_fn(shared),
        None,
    )?;
    Ok(stream)
}

struct OutputState {
    mixer: Mixer,
    rx: Receiver<Command>,
    mic_cons: HeapCons<f32>,
    monitor_prod: Option<HeapProd<f32>>,
    shared: Arc<Shared>,
    mic_buf: Vec<f32>,
    out_buf: Vec<f32>,
    mon_buf: Vec<f32>,
    max_backlog: usize,
    target_backlog: usize,
}

impl OutputState {
    fn process<T: Sample + FromSample<f32>>(&mut self, data: &mut [T], channels: usize) {
        while let Ok(cmd) = self.rx.try_recv() {
            self.mixer.handle(cmd, &self.shared);
        }
        let frames = data.len() / channels;
        if self.mic_buf.len() < frames {
            self.mic_buf.resize(frames, 0.0);
            self.out_buf.resize(frames * 2, 0.0);
            self.mon_buf.resize(frames * 2, 0.0);
        }

        // Mantém a latência do mic baixa descartando o excesso acumulado.
        let backlog = self.mic_cons.occupied_len();
        if backlog > frames + self.max_backlog {
            self.mic_cons.skip(backlog - frames - self.target_backlog);
        }
        let got = self.mic_cons.pop_slice(&mut self.mic_buf[..frames]);
        self.mic_buf[got..frames].fill(0.0);

        self.mixer.render(&self.mic_buf[..frames], &mut self.out_buf, &mut self.mon_buf, &self.shared);

        if let Some(p) = &mut self.monitor_prod {
            p.push_slice(&self.mon_buf[..frames * 2]);
        }
        write_stereo(data, channels, &self.out_buf);
    }
}

fn write_stereo<T: Sample + FromSample<f32>>(data: &mut [T], channels: usize, stereo: &[f32]) {
    for (f, frame) in data.chunks_exact_mut(channels).enumerate() {
        let (l, r) = (stereo[f * 2], stereo[f * 2 + 1]);
        if channels == 1 {
            frame[0] = T::from_sample((l + r) * 0.5);
        } else {
            frame[0] = T::from_sample(l);
            frame[1] = T::from_sample(r);
            for s in &mut frame[2..] {
                *s = T::from_sample(0.0f32);
            }
        }
    }
}

fn build_output<T>(dev: &Device, config: &StreamConfig, mut st: OutputState, channels: usize, shared: Arc<Shared>) -> Result<Stream>
where
    T: SizedSample + FromSample<f32>,
{
    let stream = dev.build_output_stream(
        config,
        move |data: &mut [T], _: &cpal::OutputCallbackInfo| st.process(data, channels),
        err_fn(shared),
        None,
    )?;
    Ok(stream)
}

struct MonitorState {
    cons: HeapCons<f32>,
    buf: Vec<f32>,
    max_backlog: usize,
    target_backlog: usize,
}

fn build_monitor<T>(dev: &Device, config: &StreamConfig, mut st: MonitorState, channels: usize, shared: Arc<Shared>) -> Result<Stream>
where
    T: SizedSample + FromSample<f32>,
{
    let stream = dev.build_output_stream(
        config,
        move |data: &mut [T], _: &cpal::OutputCallbackInfo| {
            let n = data.len() / channels * 2;
            if st.buf.len() < n {
                st.buf.resize(n, 0.0);
            }
            let backlog = st.cons.occupied_len();
            if backlog > n + st.max_backlog {
                st.cons.skip((backlog - n - st.target_backlog) & !1);
            }
            let got = st.cons.pop_slice(&mut st.buf[..n]);
            st.buf[got..n].fill(0.0);
            write_stereo(data, channels, &st.buf);
        },
        err_fn(shared),
        None,
    )?;
    Ok(stream)
}
