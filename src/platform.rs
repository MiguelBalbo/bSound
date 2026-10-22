//! Diferenças entre macOS, Windows e Linux: qual microfone virtual usar, como instalá-lo,
//! como abrir links e como se chama a área do ícone (barra de menus / bandeja).

use std::process::Command;

/// Microfone virtual recomendado no sistema atual.
pub struct VirtualMic {
    /// Nome do driver/recurso, como aparece nos textos.
    pub driver: &'static str,
    /// O nome que o usuário escolhe como microfone no Discord, Zoom, OBS...
    pub mic_name: &'static str,
    pub download_url: &'static str,
    pub download_label: &'static str,
    /// "o macOS", "o Windows", "o Linux".
    pub os: &'static str,
    /// Segundo parágrafo da introdução do assistente.
    pub intro: &'static str,
    pub install_title: &'static str,
    pub download_title: &'static str,
    pub download_text: &'static str,
}

#[cfg(target_os = "macos")]
pub const VIRTUAL: VirtualMic = VirtualMic {
    driver: "BlackHole 2ch",
    mic_name: "BlackHole 2ch",
    download_url: "https://existential.audio/blackhole/",
    download_label: "Abrir existential.audio",
    os: "o macOS",
    intro: "O BlackHole 2ch é gratuito e de código aberto. O bSound envia a mistura para ele, e nos outros apps você escolhe \"BlackHole 2ch\" como microfone.",
    install_title: "Instale o BlackHole 2ch",
    download_title: "Instalador oficial",
    download_text: "Prefere o instalador .pkg? Baixe a versão 2ch no site do projeto.",
};

#[cfg(target_os = "windows")]
pub const VIRTUAL: VirtualMic = VirtualMic {
    driver: "VB-CABLE",
    mic_name: "CABLE Output (VB-Audio Virtual Cable)",
    download_url: "https://vb-audio.com/Cable/",
    download_label: "Abrir vb-audio.com",
    os: "o Windows",
    intro: "O VB-CABLE é gratuito. O bSound envia a mistura para a saída \"CABLE Input\", e nos outros apps você escolhe \"CABLE Output\" como microfone.",
    install_title: "Instale o VB-CABLE",
    download_title: "VB-CABLE (gratuito)",
    download_text: "Baixe o pacote, extraia o zip e rode VBCABLE_Setup_x64.exe como administrador. Depois reinicie o computador.",
};

#[cfg(target_os = "linux")]
pub const VIRTUAL: VirtualMic = VirtualMic {
    driver: "microfone virtual do PulseAudio/PipeWire",
    mic_name: "bSound_Mic",
    download_url: "https://wiki.archlinux.org/title/PulseAudio/Examples",
    download_label: "Ver documentação",
    os: "o Linux",
    intro: "O bSound cria uma saída virtual no PulseAudio/PipeWire e um microfone \"bSound_Mic\" ligado a ela. Nos outros apps, escolha \"bSound_Mic\" como microfone.",
    install_title: "Crie o microfone virtual",
    download_title: "Configuração manual",
    download_text: "Prefere configurar à mão ou deixar permanente? Veja como criar saídas virtuais no PulseAudio/PipeWire.",
};

/// Como o ícone do app é chamado na interface do sistema.
#[cfg(target_os = "macos")]
pub const TRAY_AREA: &str = "barra de menus";
#[cfg(not(target_os = "macos"))]
pub const TRAY_AREA: &str = "bandeja do sistema";

/// Espaço no topo da barra lateral: no macOS os botões da janela ficam sobre ela.
#[cfg(target_os = "macos")]
pub const TITLEBAR_INSET: f32 = 46.0;
#[cfg(not(target_os = "macos"))]
pub const TITLEBAR_INSET: f32 = 24.0;

/// Um comando que instala/cria o microfone virtual automaticamente.
pub struct InstallCommand {
    pub title: &'static str,
    pub description: &'static str,
    pub button: &'static str,
    pub command: String,
}

#[cfg(target_os = "macos")]
pub fn install_command() -> Option<InstallCommand> {
    let brew = ["/opt/homebrew/bin/brew", "/usr/local/bin/brew"]
        .into_iter()
        .find(|p| std::path::Path::new(p).exists())?;
    Some(InstallCommand {
        title: "Homebrew",
        description: "Abre o Terminal com o comando abaixo. Ele pede sua senha de administrador.",
        button: "Instalar com Homebrew",
        command: format!("{brew} install --cask blackhole-2ch"),
    })
}

#[cfg(target_os = "linux")]
pub fn install_command() -> Option<InstallCommand> {
    Some(InstallCommand {
        title: "Criar agora",
        description: "Cria uma saída \"bSound\" e o microfone \"bSound_Mic\" no PulseAudio/PipeWire. Vale até reiniciar; para manter, adicione os comandos à sua sessão.",
        button: "Criar microfone virtual",
        command: format!(
            "pactl load-module module-null-sink sink_name={LINUX_SINK} sink_properties=device.description=bSound && \
             pactl load-module module-remap-source master={LINUX_SINK}.monitor source_name=bsound_mic source_properties=device.description=bSound_Mic"
        ),
    })
}

#[cfg(target_os = "windows")]
pub fn install_command() -> Option<InstallCommand> {
    None
}

/// Executa o comando de instalação. No macOS abre o Terminal (o brew pede senha);
/// no Linux roda direto, sem privilégios.
pub fn run_install(cmd: &str) -> std::io::Result<()> {
    #[cfg(target_os = "macos")]
    let status = {
        let script = format!(
            "tell application \"Terminal\"\nactivate\ndo script \"{}\"\nend tell",
            cmd.replace('\\', "\\\\").replace('"', "\\\"")
        );
        Command::new("osascript").arg("-e").arg(script).status()?
    };
    #[cfg(not(target_os = "macos"))]
    let status = Command::new("sh").arg("-c").arg(cmd).status()?;
    if status.success() {
        refresh();
        Ok(())
    } else {
        Err(std::io::Error::other("o comando falhou"))
    }
}

pub fn open_url(url: &str) {
    #[cfg(target_os = "macos")]
    let r = Command::new("open").arg(url).spawn();
    #[cfg(target_os = "windows")]
    let r = Command::new("explorer").arg(url).spawn();
    #[cfg(target_os = "linux")]
    let r = Command::new("xdg-open").arg(url).spawn();
    if let Err(e) = r {
        log::error!("não foi possível abrir {url}: {e}");
    }
}

// ---------- Detecção do dispositivo de saída virtual ----------

#[cfg(target_os = "macos")]
pub fn is_virtual(name: &str) -> bool {
    name.to_lowercase().contains("blackhole")
}

#[cfg(target_os = "windows")]
pub fn is_virtual(name: &str) -> bool {
    let n = name.to_lowercase();
    n.contains("cable input") || n.contains("vb-audio")
}

/// No Linux o cpal vê dispositivos ALSA, não as saídas do PulseAudio. A saída "pulse" é
/// direcionada para a saída virtual "bsound" com a variável PULSE_SINK.
#[cfg(target_os = "linux")]
pub const LINUX_SINK: &str = "bsound";

#[cfg(target_os = "linux")]
static LINUX_SINK_READY: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

#[cfg(target_os = "linux")]
pub fn is_virtual(name: &str) -> bool {
    name == "pulse" && LINUX_SINK_READY.load(std::sync::atomic::Ordering::Relaxed)
}

/// Atualiza o que depende do sistema (no Linux: se a saída virtual existe).
pub fn refresh() {
    #[cfg(target_os = "linux")]
    {
        let ready = Command::new("pactl")
            .args(["list", "short", "sinks"])
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).lines().any(|l| l.split_whitespace().nth(1) == Some(LINUX_SINK)))
            .unwrap_or(false);
        LINUX_SINK_READY.store(ready, std::sync::atomic::Ordering::Relaxed);
        if ready {
            // Lido pelo plugin "pulse" do ALSA ao abrir o stream.
            std::env::set_var("PULSE_SINK", LINUX_SINK);
        }
    }
}

/// Saída virtual disponível, se houver.
pub fn virtual_output(outputs: &[String]) -> Option<String> {
    outputs.iter().find(|n| is_virtual(n)).cloned()
}

// ---------- Aparência ----------

/// Se o sistema está no modo escuro. `None` quando o egui/winit já sabe responder.
#[cfg(target_os = "macos")]
pub fn system_is_dark() -> Option<bool> {
    use objc2_foundation::{ns_string, NSUserDefaults};
    Some(
        NSUserDefaults::standardUserDefaults()
            .stringForKey(ns_string!("AppleInterfaceStyle"))
            .is_some_and(|s| s.to_string().eq_ignore_ascii_case("dark")),
    )
}

#[cfg(not(target_os = "macos"))]
pub fn system_is_dark() -> Option<bool> {
    None
}
