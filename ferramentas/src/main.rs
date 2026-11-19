//! Ferramentas de desenvolvimento do Raiz OS. Não fazem parte do kernel.
//! O construtor escreve somente arquivos regulares dentro de `saida/`.
use std::env;
use std::error::Error;
use std::fs::{self, File};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::net::UdpSocket;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

type Resultado<T> = Result<T, Box<dyn Error>>;

const SETOR: usize = 512;
const SETORES_DISCO: u64 = 262_144; // 128 MiB.
const INICIO_ESP: u64 = 2_048; // Alinhamento de 1 MiB.
const FIM_ESP: u64 = SETORES_DISCO - 34;
const SETORES_ESP: u32 = (FIM_ESP - INICIO_ESP + 1) as u32;
const RESERVADOS: u32 = 32;
const ENTRADAS_GPT: usize = 128;
const TAM_ENTRADA_GPT: usize = 128;
const TAM_GPT: usize = ENTRADAS_GPT * TAM_ENTRADA_GPT;
const EOF_FAT: u32 = 0x0fff_ffff;
const TIPO_ESP: [u8; 16] = [
    0x28, 0x73, 0x2a, 0xc1, 0x1f, 0xf8, 0xd2, 0x11, 0xba, 0x4b, 0x00, 0xa0, 0xc9, 0x3e, 0xc9, 0x3b,
];
// IDs determinísticos da imagem de laboratório. Um futuro instalador deve gerar
// IDs próprios para cada instalação, não clonar estes identificadores em discos.
const ID_DISCO: [u8; 16] = [
    0x52, 0x41, 0x49, 0x5a, 0x01, 0x00, 0x00, 0x40, 0x80, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01,
];
const ID_ESP: [u8; 16] = [
    0x52, 0x41, 0x49, 0x5a, 0x02, 0x00, 0x00, 0x40, 0x80, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x02,
];

fn main() {
    if let Err(erro) = executar_comando() {
        eprintln!("Erro: {erro}");
        std::process::exit(1);
    }
}

fn executar_comando() -> Resultado<()> {
    let raiz = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .ok_or("Diretório do projeto indisponível")?
        .to_path_buf();
    let argumentos: Vec<String> = env::args().skip(1).collect();
    match argumentos.first().map(String::as_str).unwrap_or("ajuda") {
        "compilar" if argumentos.len() == 1 => compilar(&raiz, ""),
        "compilar" if argumentos.len() == 2 && argumentos[1] == "--teste-idle" => compilar(&raiz, "teste-idle"),
        "testar-idle" if argumentos.len() == 1 => testar_idle(&raiz),
        "compilar" if argumentos.len() == 2 && argumentos[1] == "--teste-ipc-bloqueante" => compilar(&raiz, "teste-ipc-bloqueante"),
        "testar-ipc-bloqueante" if argumentos.len() == 1 => testar_ipc_bloqueante(&raiz),
        "compilar" if argumentos.len() == 2 && argumentos[1] == "--teste-receber-bloqueante" => compilar(&raiz, "teste-receber-bloqueante"),
        "testar-receber-bloqueante" if argumentos.len() == 1 => testar_receber_bloqueante(&raiz),
        "compilar" if argumentos.len() == 2 && argumentos[1] == "--teste-prazo" => compilar(&raiz, "teste-prazo"),
        "testar-prazo" if argumentos.len() == 1 => testar_prazo(&raiz),
        "compilar" if argumentos.len() == 2 && argumentos[1] == "--teste-prazo-empate" => compilar(&raiz, "teste-prazo-empate"),
        "testar-prazo-empate" if argumentos.len() == 1 => testar_prazo_empate(&raiz),
        "compilar" if argumentos.len() == 2 && argumentos[1] == "--teste-prazo-idle" => compilar(&raiz, "teste-prazo-idle"),
        "testar-prazo-idle" if argumentos.len() == 1 => testar_prazo_idle(&raiz),
        "compilar" if argumentos.len() == 2 && argumentos[1] == "--teste-ciclo-espera" => compilar(&raiz, "teste-ciclo-espera"),
        "testar-ciclo-espera" if argumentos.len() == 1 => testar_ciclo_espera(&raiz),
        "compilar" if argumentos.len() == 2 && argumentos[1] == "--teste-observacao" => compilar(&raiz, "teste-observacao"),
        "testar-observacao" if argumentos.len() == 1 => testar_observacao(&raiz),
        "compilar" if argumentos.len() == 2 && argumentos[1] == "--teste-vida" => compilar(&raiz, "teste-vida"),
        "testar-vida" if argumentos.len() == 1 => testar_vida(&raiz),
        "compilar" if argumentos.len() == 2 && argumentos[1] == "--teste-quota" => compilar(&raiz, "teste-quota"),
        "testar-quota" if argumentos.len() == 1 => testar_quota(&raiz),
        "compilar" if argumentos.len() == 2 && argumentos[1] == "--teste-irq" => compilar(&raiz, "teste-irq"),
        "compilar" if argumentos.len() == 2 && argumentos[1] == "--teste-uart" => compilar(&raiz, "teste-uart"),
        "compilar" if argumentos.len() == 2 && argumentos[1] == "--teste-uart-rz025" => compilar(&raiz, "teste-uart-rz025"),
        "testar-uart-rz025" if argumentos.len() == 1 => testar_uart_rz025(&raiz),
        "compilar" if argumentos.len() == 2 && argumentos[1] == "--comparacao-vazia" => compilar(&raiz, "comparacao-vazia"),
        "testar-comparacao-vazia" if argumentos.len() == 1 => testar_comparacao_vazia(&raiz),
        "compilar" if argumentos.len() == 2 && argumentos[1] == "--teste-irq-ipc" => compilar(&raiz, "teste-irq-ipc"),
        "compilar" if argumentos.len() == 2 && argumentos[1] == "--teste-seletor-irq-ipc" => compilar(&raiz, "teste-seletor-irq-ipc"),
        "compilar" if argumentos.len() == 2 && argumentos[1] == "--teste-rx-orcamento" => compilar(&raiz, "teste-rx-orcamento"),
        "compilar" if argumentos.len() == 2 && argumentos[1] == "--teste-rx-quota" => compilar(&raiz, "teste-rx-quota"),
        "testar-rx-quota" if argumentos.len() == 1 => testar_rx_quota(&raiz),
        "testar-rx-orcamento" if argumentos.len() == 1 => testar_rx_orcamento(&raiz),
        "compilar" if argumentos.len() == 2 && argumentos[1] == "--teste-cancelamento-aplicacao" => compilar(&raiz, "teste-cancelamento-aplicacao"),
        "compilar" if argumentos.len() == 2 && argumentos[1] == "--teste-irq-morte" => compilar(&raiz, "teste-irq-morte"),
        "compilar" if argumentos.len() == 2 && argumentos[1] == "--teste-rz024-irq" => compilar(&raiz, "teste-rz024-irq"),
        "compilar" if argumentos.len() == 2 && argumentos[1] == "--teste-rz024-prazo" => compilar(&raiz, "teste-rz024-prazo"),
        "compilar" if argumentos.len() == 2 && argumentos[1] == "--teste-rz024-pressao" => compilar(&raiz, "teste-rz024-pressao"),
        "compilar" if argumentos.len() == 2 && argumentos[1] == "--teste-falha-preparacao" => compilar(&raiz, "teste-falha-preparacao"),
        "compilar" if argumentos.len() == 2 && argumentos[1] == "--teste-irq-fifo-cheia" => compilar(&raiz, "teste-irq-fifo-cheia"),
        "testar-irq-fifo-cheia" if argumentos.len() == 1 => testar_irq_fifo_cheia(&raiz),
        "testar-falha-preparacao" if argumentos.len() == 1 => testar_falha_preparacao(&raiz),
        "testar-rz024" if argumentos.len() == 1 => testar_rz024(&raiz),
        "testar-cancelamento-aplicacao" if argumentos.len() == 1 => testar_cancelamento_aplicacao(&raiz),
        "compilar" if argumentos.len() == 2 && argumentos[1] == "--teste-rz022" => compilar(&raiz, "teste-rz022"),
        "testar-rz022" if argumentos.len() == 1 => testar_rz022(&raiz),
        "testar-irq" if argumentos.len() == 1 => testar_irq(&raiz),
        "testar-uart" if argumentos.len() == 1 => testar_uart(&raiz, false),
        "testar-uart" if argumentos.len() == 2 && argumentos[1] == "--instrumentado" => testar_uart(&raiz, true),
        "testar-irq-ipc" if argumentos.len() == 1 => testar_irq_ipc(&raiz),
        "testar-seletor-irq-ipc" if argumentos.len() == 1 => testar_seletor_irq_ipc(&raiz),
        "testar-irq-morte" if argumentos.len() == 1 => testar_irq_morte(&raiz),
        "executar" if argumentos.len() == 1 ||
            (argumentos.len() == 2 && argumentos[1] == "--grafico") => {
            executar_qemu(&raiz, argumentos.len() == 2)
        }
        "testar" if argumentos.len() == 1 => testar(&raiz),
        "testar-isolamento" if argumentos.len() == 1 => testar_isolamento(&raiz),
        "testar-recuperacao" if argumentos.len() == 1 => testar_recuperacao(&raiz),
        "eco-udp" if argumentos.len() >= 2 => eco_udp(argumentos[1..].join(" ").as_bytes()),
        "verificar-imagem" if argumentos.len() == 1 => {
            let efi = fs::read(raiz.join("saida/esp/EFI/BOOT/BOOTX64.EFI"))?;
            verificar_imagem(&raiz.join("saida/raiz-os.img"), &efi)?;
            println!("Imagem íntegra: GPT principal/reserva, ESP FAT32 e BOOTX64.EFI conferidos.");
            Ok(())
        }
        "ajuda" | "--help" | "-h" => {
            println!("Raiz OS — ferramentas em Rust\n\n  cargo run -p ferramentas -- compilar [--teste-idle|--teste-ipc-bloqueante|--teste-receber-bloqueante|--teste-prazo|--teste-prazo-empate|--teste-prazo-idle|--teste-ciclo-espera|--teste-observacao|--teste-vida|--teste-quota|--teste-irq|--teste-uart|--teste-uart-rz025|--teste-irq-ipc|--teste-seletor-irq-ipc|--teste-rz022|--teste-rx-orcamento|--teste-rx-quota|--teste-cancelamento-aplicacao|--teste-irq-morte|--teste-rz024-irq|--teste-rz024-prazo|--teste-rz024-pressao|--teste-falha-preparacao|--teste-irq-fifo-cheia]\n  cargo run -p ferramentas -- testar-idle\n  cargo run -p ferramentas -- testar-ipc-bloqueante\n  cargo run -p ferramentas -- testar-receber-bloqueante\n  cargo run -p ferramentas -- testar-prazo\n  cargo run -p ferramentas -- testar-prazo-empate\n  cargo run -p ferramentas -- testar-prazo-idle\n  cargo run -p ferramentas -- testar-ciclo-espera\n  cargo run -p ferramentas -- testar-observacao\n  cargo run -p ferramentas -- testar-vida\n  cargo run -p ferramentas -- testar-quota\n  cargo run -p ferramentas -- testar-irq\n  cargo run -p ferramentas -- testar-uart [--instrumentado]\n  cargo run -p ferramentas -- testar-uart-rz025\n  cargo run -p ferramentas -- testar-irq-ipc\n  cargo run -p ferramentas -- testar-seletor-irq-ipc\n  cargo run -p ferramentas -- testar-rz022\n  cargo run -p ferramentas -- testar-rx-orcamento\n  cargo run -p ferramentas -- testar-rx-quota\n  cargo run -p ferramentas -- testar-cancelamento-aplicacao\n  cargo run -p ferramentas -- testar-irq-morte\n  cargo run -p ferramentas -- testar-rz024\n  cargo run -p ferramentas -- testar-falha-preparacao\n  cargo run -p ferramentas -- testar-irq-fifo-cheia\n  cargo run -p ferramentas -- verificar-imagem\n  cargo run -p ferramentas -- executar [--grafico]\n  cargo run -p ferramentas -- testar\n  cargo run -p ferramentas -- testar-isolamento\n  cargo run -p ferramentas -- testar-recuperacao\n  cargo run -p ferramentas -- eco-udp TEXTO\n\nVariáveis opcionais:\n  RAIZ_QEMU        executável qemu-system-x86_64\n  RAIZ_QEMU_DADOS  diretório de ROMs/dados do QEMU (-L)\n  RAIZ_OVMF        arquivo OVMF_CODE.fd / OVMF_CODE_4M.fd\n  RAIZ_OVMF_VARS   arquivo VARS.fd correspondente\n\nA imagem gerada é saida/raiz-os.img (128 MiB). Ctrl+C encerra o emulador.");
            println!("Comparação opt-in: `cargo raiz compilar --comparacao-vazia`, depois `cargo raiz testar-comparacao-vazia`; comando serial `medir-vazio`.");
            Ok(())
        }
        _ => Err("Comando inválido. Use: compilar [--comparacao-vazia|--teste-idle|--teste-ipc-bloqueante|--teste-receber-bloqueante|--teste-prazo|--teste-prazo-empate|--teste-prazo-idle|--teste-ciclo-espera|--teste-observacao|--teste-vida|--teste-quota|--teste-irq|--teste-uart|--teste-irq-ipc|--teste-seletor-irq-ipc|--teste-rz022|--teste-rx-orcamento|--teste-rx-quota|--teste-cancelamento-aplicacao|--teste-irq-morte|--teste-rz024-irq|--teste-rz024-prazo|--teste-rz024-pressao|--teste-falha-preparacao|--teste-irq-fifo-cheia], testar-comparacao-vazia, testar-idle, testar-ipc-bloqueante, testar-receber-bloqueante, testar-prazo, testar-prazo-empate, testar-prazo-idle, testar-ciclo-espera, testar-observacao, testar-vida, testar-quota, testar-irq, testar-uart [--instrumentado], testar-irq-ipc, testar-seletor-irq-ipc, testar-rz022, testar-rx-orcamento, testar-rx-quota, testar-cancelamento-aplicacao, testar-irq-morte, testar-rz024, testar-falha-preparacao, testar-irq-fifo-cheia, executar [--grafico], testar, testar-isolamento, testar-recuperacao, eco-udp TEXTO, verificar-imagem ou ajuda".into()),
    }
}

fn compilar(raiz: &Path, teste: &str) -> Resultado<()> {
    let cargo = env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let destino = raiz.join(if teste.is_empty() {
        "target".into()
    } else {
        format!("target/{teste}")
    });
    let saida = raiz.join(if teste.is_empty() {
        "saida".into()
    } else {
        format!("saida/{teste}")
    });
    println!("Compilando serviços de terminal, rede e supervisão...");
    let mut comando_servicos = Command::new(&cargo);
    comando_servicos
        .current_dir(raiz)
        .args([
            "build",
            "-p",
            "servico-terminal",
            "-p",
            "servico-rede",
            "-p",
            "servico-supervisor",
            "--target",
            "x86_64-unknown-none",
            "--release",
            "--target-dir",
        ])
        .arg(&destino);
    if teste == "teste-ipc-bloqueante" {
        comando_servicos.args([
            "--features",
            "servico-terminal/teste-ipc-bloqueante,servico-supervisor/teste-ipc-bloqueante",
        ]);
    }
    if teste == "teste-receber-bloqueante" {
        comando_servicos.args([
            "--features",
            "servico-terminal/teste-receber-bloqueante,servico-supervisor/teste-receber-bloqueante",
        ]);
    }
    if teste == "teste-prazo" {
        comando_servicos.args([
            "--features",
            "servico-terminal/teste-prazo,servico-supervisor/teste-prazo",
        ]);
    }
    if teste == "teste-prazo-empate" {
        comando_servicos.args([
            "--features",
            "servico-terminal/teste-prazo-empate,servico-rede/teste-prazo-empate,servico-supervisor/teste-prazo-empate",
        ]);
    }
    if teste == "teste-prazo-idle" {
        comando_servicos.args([
            "--features",
            "servico-terminal/teste-prazo-idle,servico-rede/teste-prazo-idle,servico-supervisor/teste-prazo-idle",
        ]);
    }
    if teste == "teste-ciclo-espera" {
        comando_servicos.args([
            "--features",
            "servico-terminal/teste-ciclo-espera,servico-rede/teste-ciclo-espera",
        ]);
    }
    if teste == "teste-observacao" {
        comando_servicos.args([
            "--features",
            "servico-terminal/teste-observacao,servico-rede/teste-observacao,servico-supervisor/teste-observacao",
        ]);
    }
    if teste == "teste-vida" {
        comando_servicos.args([
            "--features",
            "servico-terminal/teste-vida,servico-supervisor/teste-vida",
        ]);
    }
    if teste == "teste-quota" {
        comando_servicos.args([
            "--features",
            "servico-terminal/teste-quota,servico-rede/teste-quota,servico-supervisor/teste-quota",
        ]);
    }
    if teste == "teste-irq" {
        comando_servicos.args([
            "--features",
            "servico-terminal/teste-irq,servico-rede/teste-irq,servico-supervisor/teste-irq",
        ]);
    }
    if teste == "teste-rz024-irq" {
        comando_servicos.args([
            "--features",
            "servico-terminal/teste-irq,servico-rede/teste-irq,servico-supervisor/teste-irq,servico-terminal/teste-rz024",
        ]);
    }
    if teste == "teste-rz024-prazo" {
        comando_servicos.args(["--features", "servico-terminal/teste-prazo-idle,servico-rede/teste-prazo-idle,servico-supervisor/teste-prazo-idle,servico-terminal/teste-rz024"]);
    }
    if teste == "teste-rz024-pressao" {
        comando_servicos.args([
            "--features",
            "servico-terminal/teste-irq-ipc,servico-rede/teste-irq-ipc",
        ]);
    }
    if teste == "teste-uart" {
        comando_servicos.args(["--features", "servico-terminal/teste-uart"]);
    }
    if teste == "teste-uart-rz025" {
        comando_servicos.args(["--features", "servico-terminal/teste-uart-rz025"]);
    }
    if teste == "comparacao-vazia" {
        comando_servicos.args(["--features", "servico-terminal/comparacao-consultas-vazias"]);
    }
    if teste == "teste-rz022" {
        comando_servicos.args([
            "--features",
            "servico-terminal/teste-rz022,servico-rede/teste-rz022-rede,servico-supervisor/teste-rz022-supervisor",
        ]);
    }
    if teste == "teste-irq-ipc" {
        comando_servicos.args([
            "--features",
            "servico-terminal/teste-irq-ipc,servico-rede/teste-irq-ipc",
        ]);
    }
    if teste == "teste-seletor-irq-ipc" {
        comando_servicos.args([
            "--features",
            "servico-terminal/teste-seletor-irq-ipc,servico-rede/teste-seletor-irq-ipc",
        ]);
    }
    if teste == "teste-irq-morte" {
        comando_servicos.args([
            "--features",
            "servico-terminal/teste-irq-morte,servico-supervisor/teste-irq-morte",
        ]);
    }
    if teste == "teste-rx-orcamento" {
        comando_servicos.args([
            "--features",
            "servico-terminal/teste-rx-orcamento,servico-rede/teste-rx-orcamento",
        ]);
    }
    if teste == "teste-rx-quota" {
        comando_servicos.args([
            "--features",
            "servico-terminal/teste-rx-quota,servico-rede/teste-rx-quota,servico-supervisor/teste-rx-quota",
        ]);
    }
    if teste == "teste-cancelamento-aplicacao" {
        comando_servicos.args([
            "--features",
            "servico-terminal/teste-cancelamento-aplicacao,servico-rede/teste-cancelamento-aplicacao",
        ]);
    }
    if teste == "teste-irq-fifo-cheia" {
        comando_servicos.args([
            "--features",
            "servico-terminal/teste-irq-fifo-cheia,servico-rede/teste-irq-fifo-cheia,servico-supervisor/teste-irq-fifo-cheia",
        ]);
    }
    let estado = comando_servicos.status()?;
    if !estado.success() {
        return Err("A compilação dos serviços falhou".into());
    }
    let terminal = destino
        .join("x86_64-unknown-none/release/servico-terminal")
        .canonicalize()?;
    let rede = destino
        .join("x86_64-unknown-none/release/servico-rede")
        .canonicalize()?;
    let supervisor = destino
        .join("x86_64-unknown-none/release/servico-supervisor")
        .canonicalize()?;

    println!("Compilando microkernel com os ELFs dos serviços incorporados...");
    let mut comando_nucleo = Command::new(&cargo);
    comando_nucleo
        .current_dir(raiz)
        .args([
            "build",
            "-p",
            "nucleo",
            "--target",
            "x86_64-unknown-none",
            "--release",
            "--target-dir",
        ])
        .arg(&destino)
        .env("RAIZ_TERMINAL", &terminal)
        .env("RAIZ_REDE", &rede)
        .env("RAIZ_SUPERVISOR", &supervisor);
    if !teste.is_empty()
        && teste != "teste-quota"
        && teste != "teste-rz022"
        && teste != "teste-irq-ipc"
        && teste != "teste-seletor-irq-ipc"
        && teste != "teste-irq-morte"
        && teste != "teste-rx-orcamento"
        && teste != "teste-rx-quota"
        && teste != "teste-cancelamento-aplicacao"
        && teste != "teste-irq-fifo-cheia"
        && teste != "teste-uart-rz025"
        && teste != "comparacao-vazia"
        && teste != "teste-prazo-empate"
        && teste != "teste-falha-preparacao"
        && !teste.starts_with("teste-rz024-")
    {
        comando_nucleo.args([
            "--features",
            if teste == "teste-observacao" {
                "teste-ciclo-espera"
            } else {
                teste
            },
        ]);
    }
    if teste == "teste-rz022" {
        comando_nucleo.args(["--features", "teste-rz022"]);
    }
    if teste == "teste-irq-ipc" {
        comando_nucleo.args(["--features", "teste-irq-ipc"]);
    }
    if teste == "teste-seletor-irq-ipc" {
        comando_nucleo.args(["--features", "teste-seletor-irq-ipc"]);
    }
    if teste == "teste-irq-morte" {
        comando_nucleo.args(["--features", "teste-irq-morte"]);
    }
    if teste == "teste-rz024-irq" {
        comando_nucleo.args(["--features", "teste-irq,teste-rz024"]);
    }
    if teste == "teste-rz024-prazo" {
        comando_nucleo.args(["--features", "teste-prazo-idle,teste-rz024"]);
    }
    if teste == "teste-rz024-pressao" {
        comando_nucleo.args(["--features", "teste-irq-ipc,teste-rz024"]);
    }
    if teste == "teste-falha-preparacao" {
        comando_nucleo.args(["--features", "teste-falha-preparacao"]);
    }
    if teste == "teste-irq-fifo-cheia" {
        comando_nucleo.args(["--features", "teste-irq-fifo-cheia"]);
    }
    if teste == "teste-prazo-empate" {
        comando_nucleo.args(["--features", "teste-prazo-empate"]);
    }
    if teste == "teste-rx-quota" {
        comando_nucleo.args(["--features", "teste-rx-quota"]);
    }
    let estado = comando_nucleo.status()?;
    if !estado.success() {
        return Err("A compilação do microkernel falhou".into());
    }

    let kernel = destino
        .join("x86_64-unknown-none/release/nucleo")
        .canonicalize()?;
    println!("Compilando carregador UEFI com o kernel incorporado...");
    let estado = Command::new(&cargo)
        .current_dir(raiz)
        .args([
            "build",
            "-p",
            "inicializador",
            "--target",
            "x86_64-unknown-uefi",
            "--release",
            "--target-dir",
        ])
        .arg(&destino)
        .env("RAIZ_KERNEL", &kernel)
        .status()?;
    if !estado.success() {
        return Err("A compilação do carregador falhou".into());
    }

    let efi = fs::read(destino.join("x86_64-unknown-uefi/release/inicializador.efi"))?;
    if efi.len() < 64 || &efi[..2] != b"MZ" {
        return Err("O carregador produzido não contém o cabeçalho PE esperado".into());
    }
    let esp = saida.join("esp/EFI/BOOT");
    fs::create_dir_all(&esp)?;
    fs::write(esp.join("BOOTX64.EFI"), &efi)?;
    let imagem = saida.join("raiz-os.img");
    criar_imagem(&imagem, &efi)?;
    verificar_imagem(&imagem, &efi)?;
    println!(
        "Pronto: {} (128 MiB; carregador de {} bytes).",
        imagem.display(),
        efi.len()
    );
    println!("GPT, FAT32 e bytes do carregador foram conferidos após a gravação.");
    Ok(())
}

fn executar_qemu(raiz: &Path, grafico: bool) -> Resultado<()> {
    let mut comando = preparar_qemu(raiz, grafico, false)?;
    println!("Iniciando Raiz OS em QEMU/TCG. Ctrl+C encerra o emulador.");
    println!("UDP local: 127.0.0.1:17777 → 10.0.2.15:7777.");
    let estado = comando.status()?;
    if estado.success() {
        Ok(())
    } else {
        Err(format!("QEMU terminou com estado {estado}").into())
    }
}

fn preparar_qemu(raiz: &Path, grafico: bool, teste: bool) -> Resultado<Command> {
    preparar_qemu_em(raiz, grafico, teste, "")
}

fn preparar_qemu_em(raiz: &Path, grafico: bool, teste: bool, variante: &str) -> Resultado<Command> {
    let saida = raiz.join(if variante.is_empty() {
        "saida".into()
    } else {
        format!("saida/{variante}")
    });
    let imagem = saida.join("raiz-os.img");
    if !imagem.is_file() {
        if !variante.is_empty() {
            return Err(format!("Imagem ausente; execute cargo raiz compilar --{variante}").into());
        }
        compilar(raiz, "")?;
    }
    let efi = fs::read(saida.join("esp/EFI/BOOT/BOOTX64.EFI"))?;
    verificar_imagem(&imagem, &efi)?;
    let firmware = localizar_firmware()?;
    let qemu = env::var_os("RAIZ_QEMU").unwrap_or_else(|| "qemu-system-x86_64".into());
    let mut comando = Command::new(qemu);
    comando
        .current_dir(raiz)
        .args(["-machine", "pc", "-accel", "tcg", "-m", "256M", "-smp", "1"])
        .arg("-drive")
        .arg(format!(
            "if=pflash,format=raw,unit=0,readonly=on,file={}",
            opcao_caminho(&firmware)
        ))
        .arg("-drive")
        .arg(format!(
            "if=ide,format=raw,snapshot=on,file={}",
            opcao_caminho(&imagem)
        ))
        .args([
            "-device",
            "e1000,netdev=rede,mac=52:54:00:12:34:56",
            "-netdev",
        ]);
    let rede = if variante == "teste-rx-orcamento" {
        "user,id=rede,hostfwd=udp:127.0.0.1:17777-10.0.2.15:7777,hostfwd=udp:127.0.0.1:17778-10.0.2.15:7778"
    } else if variante == "teste-rx-quota" {
        "user,id=rede,hostfwd=udp:127.0.0.1:17777-10.0.2.15:7777,hostfwd=udp:127.0.0.1:17779-10.0.2.15:7779"
    } else {
        "user,id=rede,hostfwd=udp:127.0.0.1:17777-10.0.2.15:7777"
    };
    comando
        .arg(rede)
        .args(["-serial", "stdio", "-monitor", "none", "-no-reboot"]);
    if let Some(dados) = env::var_os("RAIZ_QEMU_DADOS") {
        comando.arg("-L").arg(dados);
    }
    if !grafico {
        comando.args(["-display", "none"]);
    }
    if let Some(variaveis) = localizar_variaveis(&firmware) {
        let copia = saida.join(if teste {
            "OVMF_VARS_TESTE.fd"
        } else {
            "OVMF_VARS.fd"
        });
        if teste || !copia.exists() {
            fs::copy(&variaveis, &copia)?;
        }
        comando.arg("-drive").arg(format!(
            "if=pflash,format=raw,unit=1,file={}",
            opcao_caminho(&copia)
        ));
    }
    Ok(comando)
}

/// Mata e recolhe o processo também em retorno antecipado ou unwinding.
struct ProcessoQemu(Child);

impl ProcessoQemu {
    fn encerrar(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

impl Drop for ProcessoQemu {
    fn drop(&mut self) {
        self.encerrar();
    }
}

enum EventoSaida {
    Trecho { serial: bool, dados: Vec<u8> },
    Erro(String),
}

struct Captura {
    eventos: Receiver<EventoSaida>,
    serial: Vec<u8>,
    erros: Vec<u8>,
    arquivo_serial: File,
    arquivo_erros: File,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ResultadoComparacaoVazia {
    inicio: u64,
    fim: u64,
    decorrido: u64,
    consultas_ipc_vazias: u64,
    voltas_sem_trabalho: u64,
}

impl Captura {
    fn registrar(&mut self, evento: EventoSaida) -> Resultado<()> {
        match evento {
            EventoSaida::Trecho { serial, dados } => {
                let (registro, arquivo) = if serial {
                    (&mut self.serial, &mut self.arquivo_serial)
                } else {
                    (&mut self.erros, &mut self.arquivo_erros)
                };
                if registro.len() + dados.len() > 8 * 1024 * 1024 {
                    return Err("O emulador excedeu o limite de 8 MiB de log por fluxo".into());
                }
                arquivo.write_all(&dados)?;
                registro.extend_from_slice(&dados);
                Ok(())
            }
            EventoSaida::Erro(erro) => Err(erro.into()),
        }
    }

    fn drenar(&mut self) -> Resultado<()> {
        while let Ok(evento) = self.eventos.try_recv() {
            self.registrar(evento)?;
        }
        Ok(())
    }

    fn aguardar(
        &mut self,
        processo: &mut ProcessoQemu,
        marcador: &str,
        desde: usize,
        prazo: Duration,
    ) -> Resultado<()> {
        self.aguardar_condicao(processo, marcador, desde, prazo, |bytes| {
            bytes
                .windows(marcador.len())
                .any(|w| w == marcador.as_bytes())
        })
    }

    fn aguardar_linha(
        &mut self,
        processo: &mut ProcessoQemu,
        linha: &str,
        desde: usize,
        prazo: Duration,
    ) -> Resultado<()> {
        self.aguardar_condicao(processo, linha, desde, prazo, |bytes| {
            String::from_utf8_lossy(bytes)
                .split_inclusive('\n')
                .any(|l| linha_corresponde(l, linha))
        })
    }

    fn aguardar_condicao(
        &mut self,
        processo: &mut ProcessoQemu,
        marcador: &str,
        desde: usize,
        prazo: Duration,
        satisfaz: impl Fn(&[u8]) -> bool,
    ) -> Resultado<()> {
        let limite = Instant::now() + prazo;
        loop {
            self.drenar()?;
            if self.serial.get(desde..).is_some_and(&satisfaz) {
                return Ok(());
            }
            if let Some(estado) = processo.0.try_wait()? {
                self.drenar()?;
                return Err(format!(
                    "QEMU terminou ({estado}) antes do marcador {marcador:?}. stderr: {}",
                    String::from_utf8_lossy(&self.erros)
                )
                .into());
            }
            let restante = limite.saturating_duration_since(Instant::now());
            if restante.is_zero() {
                return Err(format!(
                    "Tempo esgotado aguardando {marcador:?} após {} segundos",
                    prazo.as_secs()
                )
                .into());
            }
            match self
                .eventos
                .recv_timeout(restante.min(Duration::from_millis(100)))
            {
                Ok(evento) => self.registrar(evento)?,
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    return Err("Os fluxos do emulador foram encerrados".into());
                }
            }
        }
    }
}

fn leitor_saida<R: Read + Send + 'static>(
    mut origem: R,
    serial: bool,
    destino: mpsc::Sender<EventoSaida>,
) -> JoinHandle<()> {
    thread::spawn(move || {
        let mut bloco = [0u8; 4096];
        loop {
            match origem.read(&mut bloco) {
                Ok(0) => break,
                Ok(tamanho) => {
                    if destino
                        .send(EventoSaida::Trecho {
                            serial,
                            dados: bloco[..tamanho].to_vec(),
                        })
                        .is_err()
                    {
                        break;
                    }
                }
                Err(erro) if erro.kind() == io::ErrorKind::Interrupted => continue,
                Err(erro) => {
                    let _ = destino.send(EventoSaida::Erro(format!(
                        "Falha lendo saída do emulador: {erro}"
                    )));
                    break;
                }
            }
        }
    })
}

fn testar_ipc_bloqueante(raiz: &Path) -> Resultado<()> {
    let instante = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_nanos();
    let pasta = raiz.join(format!("evidencias/teste-ipc-bloqueante-{instante}"));
    fs::create_dir(&pasta)?;
    let resultado = executar_teste_ipc_bloqueante(
        raiz,
        File::create(pasta.join("serial.log"))?,
        File::create(pasta.join("qemu-stderr.log"))?,
    );
    fs::write(
        pasta.join("resultado.txt"),
        match &resultado {
            Ok(()) => "APROVADO. No ensaio opt-in em QEMU, a rede tentou confirmar a sequência IRQ4 real e recebeu PERMISSAO; em seguida enviou um IPC com codigo=classe_evento::IRQ e a mesma sequência. O terminal recebeu a resposta como IPC enquanto conservava a sequência real, e a confirmação autorizada posterior da sequência teve sucesso. Isso cobre a autorização da chamada e a distinção entre classe de retorno do kernel e código de mensagem neste cenário; não é uma prova geral contra serviços comprometidos.".to_owned(),
            Err(e) => format!("FALHOU: {e}"),
        },
    )?;
    println!("Evidencias: {}", pasta.display());
    resultado
}

fn testar_uart(raiz: &Path, instrumentado: bool) -> Resultado<()> {
    let instante = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_nanos();
    let pasta = raiz.join(format!("evidencias/teste-uart-{instante}"));
    fs::create_dir(&pasta)?;
    let resultado = executar_teste_uart(
        raiz,
        File::create(pasta.join("serial.log"))?,
        File::create(pasta.join("qemu-stderr.log"))?,
        instrumentado,
    );
    fs::write(
        pasta.join("resultado.txt"),
        match &resultado {
            Ok(()) => {
                format!("APROVADO: comandos e respostas completos; instrumentado={instrumentado}.")
            }
            Err(e) => format!("FALHOU: {e}"),
        },
    )?;
    println!("Evidencias: {}", pasta.display());
    resultado
}

fn testar_uart_rz025(raiz: &Path) -> Resultado<()> {
    let instante = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_nanos();
    let pasta = raiz.join(format!("evidencias/teste-uart-rz025-{instante}"));
    fs::create_dir(&pasta)?;
    let resultado = executar_teste_uart_rz025(
        raiz,
        File::create(pasta.join("serial.log"))?,
        File::create(pasta.join("qemu-stderr.log"))?,
        File::create(pasta.join("estimulo-host.log"))?,
    );
    fs::write(
        pasta.join("resultado.txt"),
        match &resultado {
            Ok(()) => "# Ensaio T03-16 — recepção UART por fase\n\nResultado: **APROVADO** no perfil QEMU/TCG, pc/i440fx, 1 CPU. `estimulo-host.log` registra os bytes submetidos ao stdin serial do QEMU após flush; `serial.log` registra os bytes lidos pelo terminal no RBR, com fase, ID e ordem, causas IIR amostradas antes da leitura e o bit OE do LSR por fase. O byte isolado e a rajada limitada são avaliados separadamente. O limiar RX configurado é 14 bytes (FCR 0xc7). OE é um indicador observado, não uma contagem de bytes perdidos.\n\nLimite: submissão pelo host não prova entrega pelo QEMU, e uma divergência entre submissão e leitura RBR não localiza por si só onde ocorreu perda. O ensaio não infere overrun pelo tamanho da rajada, não demonstra perda zero geral, taxa máxima ou comportamento de UART física. Logs: `serial.log`, `qemu-stderr.log` e `estimulo-host.log`.\n".to_owned(),
            Err(e) => format!("# Ensaio T03-16 — recepção UART por fase\n\nResultado: **FALHOU**. `{e}`\n\n`estimulo-host.log` registra submissões ao stdin serial do QEMU; não atribuímos divergências a perda interna da UART sem evidência específica. Logs: `serial.log`, `qemu-stderr.log` e `estimulo-host.log`.\n"),
        },
    )?;
    println!("Evidencias: {}", pasta.display());
    resultado
}

fn testar_comparacao_vazia(raiz: &Path) -> Resultado<()> {
    let instante = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_nanos();
    let pasta = raiz
        .join("evidencias")
        .join(format!("teste-comparacao-vazia-{instante}"));
    fs::create_dir_all(&pasta)?;
    fs::write(
        pasta.join("comando.log"),
        "cargo raiz compilar --comparacao-vazia\ncargo raiz testar-comparacao-vazia\ncomando serial: medir-vazio\n",
    )?;
    let inicio_execucao = Instant::now();
    let resultado = executar_comparacao_vazia(
        raiz,
        File::create(pasta.join("serial.log"))?,
        File::create(pasta.join("qemu-stderr.log"))?,
    );
    let relatorio = match &resultado {
        Ok(medicao) => format!(
            "# Comparação de consultas vazias — revisão atual RZ-025\n\nResultado: **APROVADO**. Perfil QEMU/TCG, `pc`/i440fx, 1 CPU, 256 MiB, OVMF e e1000. Janela de {} tiques (`inicio={}`, `fim={}`, `decorrido={}`).\n\nMétricas CPL3, com as mesmas definições da fixture de base 0.2: {} chamadas reais `receber()` que retornaram `None`, e {} voltas completas sem IPC consumido nem byte serial/PS2 lido. A amostra encerrou somente no delta exato de 100 tiques; ultrapassar a janela resulta em falha. No loop atual, IPC retirado pelo multiplexador fica em `ipc_pendente` e é marcado como trabalho ao ser consumido (também marcamos a atividade ao retornar IPC no multiplexador). O loop 0.2 não tem esse estágio de entrega pendente.\n\nLimites: `receber()` retorna `None` para consulta sem mensagem sem distinguir outras falhas não decodificadas como mensagem. A contagem de voltas sem trabalho não equivale a ciclos de CPU, utilização, energia, latência ou desempenho: o terminal atual usa IRQ4/espera multiplexada, e o loop 0.2 consulta/polla de forma diferente. As métricas identificam pontos do loop observados na janela, não custo equivalente entre revisões.\n\nEvidências: `serial.log`, `qemu-stderr.log` e `comando.log`. Duração do harness: {:.3} s.\n",
            medicao.decorrido,
            medicao.inicio,
            medicao.fim,
            medicao.decorrido,
            medicao.consultas_ipc_vazias,
            medicao.voltas_sem_trabalho,
            inicio_execucao.elapsed().as_secs_f64()
        ),
        Err(erro) => format!(
            "# Comparação de consultas vazias — revisão atual RZ-025\n\nResultado: **FALHOU**.\n\nA imagem opt-in não foi recompilada pelo harness. Falha: `{erro}`\n\nEsta execução não sustenta métricas comparáveis. Os pontos observados na revisão atual usam IRQ4/espera multiplexada, ao contrário do loop de polling base 0.2. Evidências parciais: `serial.log`, `qemu-stderr.log` e `comando.log`. Duração: {:.3} s.\n",
            inicio_execucao.elapsed().as_secs_f64()
        ),
    };
    fs::write(pasta.join("resultado.txt"), relatorio)?;
    println!("Evidências: {}", pasta.display());
    resultado.map(|_| ())
}

fn executar_comparacao_vazia(
    raiz: &Path,
    arquivo_serial: File,
    arquivo_erros: File,
) -> Resultado<ResultadoComparacaoVazia> {
    conferir(
        raiz.join("saida/comparacao-vazia/raiz-os.img").is_file()
            && raiz
                .join("saida/comparacao-vazia/esp/EFI/BOOT/BOOTX64.EFI")
                .is_file(),
        "Imagem opt-in ausente; compile primeiro com `cargo raiz compilar --comparacao-vazia`",
    )?;
    let reserva = UdpSocket::bind("127.0.0.1:17777")
        .map_err(|e| format!("UDP 127.0.0.1:17777 indisponível: {e}"))?;
    let mut comando = preparar_qemu_em(raiz, false, true, "comparacao-vazia")?;
    comando
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    drop(reserva);
    let mut processo = ProcessoQemu(comando.spawn()?);
    let mut entrada = processo
        .0
        .stdin
        .take()
        .ok_or("stdin do emulador indisponível")?;
    let saida = processo
        .0
        .stdout
        .take()
        .ok_or("stdout do emulador indisponível")?;
    let erros = processo
        .0
        .stderr
        .take()
        .ok_or("stderr do emulador indisponível")?;
    let (emissor, receptor) = mpsc::channel();
    let leitores = [
        leitor_saida(saida, true, emissor.clone()),
        leitor_saida(erros, false, emissor),
    ];
    let mut captura = Captura {
        eventos: receptor,
        serial: Vec::new(),
        erros: Vec::new(),
        arquivo_serial,
        arquivo_erros,
    };
    let resultado = (|| -> Resultado<ResultadoComparacaoVazia> {
        captura.aguardar_linha(
            &mut processo,
            "[teste] TERMINAL_PRONTO",
            0,
            Duration::from_secs(30),
        )?;
        aguardar_evento(
            &mut captura,
            &mut processo,
            0,
            "[rede] SERVICO_PRONTO",
            &["CPL=3", "geracao=1"],
        )?;
        aguardar_evento(
            &mut captura,
            &mut processo,
            0,
            "[supervisor] SERVICO_PRONTO",
            &["CPL=3"],
        )?;
        conferir(
            captura
                .serial
                .windows(b"EXIT_BOOT_SERVICES_OK".len())
                .any(|w| w == b"EXIT_BOOT_SERVICES_OK"),
            "O boot não mostrou EXIT_BOOT_SERVICES_OK",
        )?;
        captura.drenar()?;
        let inicio_comando = enviar_comando(&mut entrada, &mut captura, "medir-vazio\n")?;
        captura.aguardar_condicao(
            &mut processo,
            "marcador ARMADA ou FALHA completo",
            inicio_comando,
            Duration::from_secs(15),
            |bytes| {
                linha_com_prefixo_completa(bytes, "[comparacao-vazia] ARMADA")
                    || linha_com_prefixo_completa(bytes, "[comparacao-vazia] FALHA")
            },
        )?;
        captura.aguardar_condicao(
            &mut processo,
            "RESULTADO/FALHA da janela completa",
            inicio_comando,
            Duration::from_secs(15),
            |bytes| {
                linha_com_prefixo_completa(bytes, "[comparacao-vazia] RESULTADO")
                    || linha_com_prefixo_completa(bytes, "[comparacao-vazia] FALHA")
                    || linha_com_prefixo_completa(bytes, "[microkernel] FALHA_NUCLEO")
                    || linha_com_prefixo_completa(bytes, "[microkernel] SERVICO_FALHOU")
                    || linha_com_prefixo_completa(bytes, "[microkernel] SERVICO_FINALIZADO")
            },
        )?;
        captura.drenar()?;
        validar_log_comparacao_vazia(&captura.serial)
    })();
    drop(entrada);
    processo.encerrar();
    let mut erro_finalizacao = None;
    for leitor in leitores {
        if leitor.join().is_err() {
            erro_finalizacao = Some("Falha na thread de captura do emulador".to_owned());
        }
    }
    if let Err(erro) = captura.drenar() {
        erro_finalizacao = Some(erro.to_string());
    }
    captura.arquivo_serial.flush()?;
    captura.arquivo_erros.flush()?;
    match resultado {
        Err(erro) => Err(erro),
        Ok(medicao) => match erro_finalizacao {
            Some(erro) => Err(erro.into()),
            None => Ok(medicao),
        },
    }
}

fn executar_teste_uart_rz025(
    raiz: &Path,
    serial: File,
    erros: File,
    mut estimulo: File,
) -> Resultado<()> {
    let reserva = UdpSocket::bind("127.0.0.1:17777")?;
    let mut comando = preparar_qemu_em(raiz, false, true, "teste-uart-rz025")?;
    comando
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    drop(reserva);
    let mut processo = ProcessoQemu(comando.spawn()?);
    let mut entrada = processo.0.stdin.take().ok_or("stdin indisponivel")?;
    let saida = processo.0.stdout.take().ok_or("stdout indisponivel")?;
    let stderr = processo.0.stderr.take().ok_or("stderr indisponivel")?;
    let (emissor, receptor) = mpsc::channel();
    let leitores = [
        leitor_saida(saida, true, emissor.clone()),
        leitor_saida(stderr, false, emissor),
    ];
    let mut captura = Captura {
        eventos: receptor,
        serial: Vec::new(),
        erros: Vec::new(),
        arquivo_serial: serial,
        arquivo_erros: erros,
    };
    let mut registro_host = String::new();
    let resultado = (|| -> Resultado<()> {
        captura.aguardar(
            &mut processo,
            "[teste] TERMINAL_PRONTO",
            0,
            Duration::from_secs(30),
        )?;
        aguardar_rede_geracao(&mut captura, &mut processo, 0, 1)?;
        captura.aguardar(
            &mut processo,
            "[supervisor] SERVICO_PRONTO",
            0,
            Duration::from_secs(30),
        )?;

        let inicio = enviar_comando(&mut entrada, &mut captura, "teste-uart-rz025\n")?;
        captura.aguardar_linha(
            &mut processo,
            "[teste-uart-rz025] FASE_PRONTA fase=UNICO",
            inicio,
            Duration::from_secs(10),
        )?;
        let byte_unico = [0x55u8];
        entrada.write_all(&byte_unico)?;
        entrada.flush()?;
        registro_host.push_str("fase=UNICO submitted=55 quantidade=1\n");
        estimulo.write_all(b"fase=UNICO submitted=55 quantidade=1\n")?;
        estimulo.flush()?;
        captura.aguardar(
            &mut processo,
            "[teste-uart-rz025] FASE_FIM fase=UNICO",
            inicio,
            Duration::from_secs(10),
        )?;

        let inicio_rajada =
            enviar_comando(&mut entrada, &mut captura, "teste-uart-rz025-rajada\n")?;
        captura.aguardar_linha(
            &mut processo,
            "[teste-uart-rz025] FASE_PRONTA fase=RAJADA",
            inicio_rajada,
            Duration::from_secs(10),
        )?;
        let rajada = [0x31u8, 0x32, 0x33, 0x34, 0x35, 0x36, 0x37, 0x38];
        entrada.write_all(&rajada)?;
        entrada.flush()?;
        registro_host.push_str("fase=RAJADA submitted=31,32,33,34,35,36,37,38 quantidade=8\n");
        estimulo.write_all(b"fase=RAJADA submitted=31,32,33,34,35,36,37,38 quantidade=8\n")?;
        estimulo.flush()?;
        captura.aguardar(
            &mut processo,
            "[teste-uart-rz025] FASE_FIM fase=RAJADA",
            inicio_rajada,
            Duration::from_secs(15),
        )?;
        captura.aguardar_linha(
            &mut processo,
            "[teste-uart-rz025] FIM CPL3",
            inicio_rajada,
            Duration::from_secs(5),
        )?;
        validar_log_uart_rz025(&captura.serial)?;
        validar_estimulo_host_rz025(registro_host.as_bytes())?;
        Ok(())
    })();
    processo.encerrar();
    for leitor in leitores {
        let _ = leitor.join();
    }
    captura.drenar()?;
    resultado?;
    Ok(())
}

fn validar_log_uart_rz025(serial: &[u8]) -> Resultado<()> {
    let texto = std::str::from_utf8(serial)?;
    let esperados: [&[u8]; 2] = [&[0x55], &[0x31, 0x32, 0x33, 0x34, 0x35, 0x36, 0x37, 0x38]];
    let mut pronta = Vec::new();
    let mut leituras: [Vec<(u64, u8, usize)>; 2] = [Vec::new(), Vec::new()];
    let mut causas: [Vec<(u8, usize)>; 2] = [Vec::new(), Vec::new()];
    let mut irq4: [Vec<u64>; 2] = [Vec::new(), Vec::new()];
    let mut amostras_iir: [Vec<(u64, u64, usize)>; 2] = [Vec::new(), Vec::new()];
    let mut finais: [Option<(u64, u64, u64, u64, u64)>; 2] = [None, None];
    let mut ultima_sequencia_irq = 0u64;
    let mut configuracoes = 0;
    let mut fim = 0;
    let mut estado = 0u8;
    let mut ultima_posicao_final = 0usize;
    for (posicao, linha_crlf) in texto.split_inclusive('\n').enumerate() {
        if !linha_crlf.ends_with('\n') && linha_crlf.contains("[teste-uart-rz025]") {
            return Err("Marcador UART final incompleto".into());
        }
        let linha = linha_crlf.trim_end_matches(['\r', '\n']);
        let campos: Vec<_> = linha.split_whitespace().collect();
        if campos.first() != Some(&"[teste-uart-rz025]") {
            continue;
        }
        match campos.get(1).copied() {
            Some("CONFIG")
                if campos.as_slice()
                    == ["[teste-uart-rz025]", "CONFIG", "FCR=0xc7", "RX_LIMIAR=14"] =>
            {
                configuracoes += 1;
                if configuracoes != 1 || estado != 0 {
                    return Err("Configuração UART duplicada ou tardia".into());
                }
            }
            Some("FASE_PRONTA") if campos.len() == 3 => {
                let fase = indice_fase_uart(campos[2])?;
                let estado_esperado = if fase == 0 { 0 } else { 2 };
                if estado != estado_esperado {
                    return Err("FASE_PRONTA fora da ordem".into());
                }
                estado = if fase == 0 { 1 } else { 3 };
                pronta.push(fase);
            }
            Some("RBR") if campos.len() == 5 => {
                let fase = indice_fase_uart(campos[2])?;
                if estado != if fase == 0 { 1 } else { 3 } {
                    return Err("Leitura RBR fora da fase pronta".into());
                }
                let id = campo_decimal_uart(campos[3], "id=")?;
                let valor = campo_hex_uart(campos[4], "valor=")?;
                if finais[fase].is_some() {
                    return Err("Leitura RBR veio após fim da fase".into());
                }
                leituras[fase].push((id, valor, posicao));
            }
            Some("IRQ4") if campos.len() == 8 => {
                let fase = indice_fase_uart(campos[2])?;
                if estado != if fase == 0 { 1 } else { 3 } {
                    return Err("IRQ4 fora da fase pronta".into());
                }
                let sequencia = campo_decimal_uart(campos[3], "seq=")?;
                let rbr_antes = campo_decimal_uart(campos[4], "rbr_antes=")?;
                let iir_bruto = campo_hex_uart(campos[5], "iir_raw=")?;
                let pending_bit = campo_decimal_uart(campos[6], "iir_pending_bit=")?;
                let causa = campo_hex_uart(campos[7], "causa=")?;
                if sequencia == 0 || sequencia <= ultima_sequencia_irq {
                    return Err("Sequência IRQ4 nula, duplicada ou reordenada".into());
                }
                ultima_sequencia_irq = sequencia;
                if rbr_antes != leituras[fase].len() as u64
                    || amostras_iir[fase]
                        .last()
                        .is_some_and(|(_, anterior, _)| rbr_antes <= *anterior)
                {
                    return Err("Amostra IIR não corresponde à posição antes do próximo RBR".into());
                }
                if pending_bit != 0 || iir_bruto & 1 != 0 {
                    return Err("IIR indica ausência de interrupção pendente".into());
                }
                if causa != 0x04 && causa != 0x0c {
                    return Err("IIR não identifica recepção disponível ou timeout RX".into());
                }
                if iir_bruto & 0x0e != causa {
                    return Err("Causa decodificada diverge do IIR bruto".into());
                }
                irq4[fase].push(sequencia);
                causas[fase].push((causa, posicao));
                amostras_iir[fase].push((sequencia, rbr_antes, posicao));
            }
            Some("FASE_FIM") if campos.len() == 8 => {
                let fase = indice_fase_uart(campos[2])?;
                if estado != if fase == 0 { 1 } else { 3 } {
                    return Err("FASE_FIM fora da ordem".into());
                }
                if finais[fase].is_some() {
                    return Err("FASE_FIM UART duplicado".into());
                }
                let rbr = campo_decimal_uart(campos[3], "rbr=")?;
                let oe = campo_decimal_uart(campos[4], "lsr_oe=")?;
                let contagem_irq = campo_decimal_uart(campos[5], "irq4=")?;
                let rda = campo_decimal_uart(campos[6], "iir_rda=")?;
                let timeout = campo_decimal_uart(campos[7], "iir_timeout=")?;
                if oe > 1 {
                    return Err("LSR OE deve ser um indicador booleano".into());
                }
                let observado = &leituras[fase];
                let contagem_esperada = esperados[fase].len();
                if rbr != contagem_esperada as u64 || observado.len() != contagem_esperada {
                    return Err("Quantidade de leituras RBR divergente".into());
                }
                for (indice, (id, valor, _)) in observado.iter().enumerate() {
                    if *id != (indice + 1) as u64 || *valor != esperados[fase][indice] {
                        return Err("ID, ordem, duplicata, omissão ou valor RBR divergente".into());
                    }
                }
                if let (Some((_, primeira_iir)), Some((_, _, primeira_rbr))) =
                    (causas[fase].first(), observado.first())
                {
                    if primeira_iir >= primeira_rbr {
                        return Err("IIR não foi registrado antes da primeira leitura RBR".into());
                    }
                }
                if contagem_irq != irq4[fase].len() as u64
                    || contagem_irq == 0
                    || causas[fase].len() != irq4[fase].len()
                    || amostras_iir[fase].len() != irq4[fase].len()
                {
                    return Err("Resumo IRQ4/IIR diverge das amostras da fase".into());
                }
                if amostras_iir[fase].first().map(|(_, antes, _)| *antes) != Some(0) {
                    return Err("Fase não observou causa IRQ antes do primeiro RBR".into());
                }
                for (_, rbr_antes, posicao_iir) in &amostras_iir[fase] {
                    let proximo = observado
                        .get(*rbr_antes as usize)
                        .ok_or("Amostra IIR sem leitura RBR subsequente")?;
                    if *posicao_iir >= proximo.2 {
                        return Err("Amostra IIR foi reordenada após o RBR correspondente".into());
                    }
                }
                let contagem_rda = causas[fase]
                    .iter()
                    .filter(|&&(causa, _)| causa == 0x04)
                    .count() as u64;
                let contagem_timeout = causas[fase]
                    .iter()
                    .filter(|&&(causa, _)| causa == 0x0c)
                    .count() as u64;
                if (rda, timeout) != (contagem_rda, contagem_timeout) {
                    return Err("Resumo IIR diverge das amostras por IRQ".into());
                }
                finais[fase] = Some((rbr, oe, contagem_irq, rda, timeout));
                estado = if fase == 0 { 2 } else { 4 };
                ultima_posicao_final = posicao;
            }
            Some("FIM") if campos.as_slice() == ["[teste-uart-rz025]", "FIM", "CPL3"] => {
                if estado != 4 {
                    return Err("FIM antes do encerramento das duas fases".into());
                }
                fim += 1;
                if posicao < ultima_posicao_final {
                    return Err("FIM precede fechamento de fase".into());
                }
            }
            Some("FALHA_ARMA_FASE" | "FALHA_LIMITE") => {
                return Err("A fixture UART sinalizou falha".into());
            }
            _ => return Err(format!("Marcador UART malformado: {linha}").into()),
        }
    }
    if configuracoes != 1
        || pronta != [0, 1]
        || finais.iter().any(Option::is_none)
        || fim != 1
        || estado != 4
    {
        return Err("Marcadores essenciais UART ausentes, duplicados ou fora de ordem".into());
    }
    Ok(())
}

fn indice_fase_uart(campo: &str) -> Resultado<usize> {
    match campo {
        "fase=UNICO" => Ok(0),
        "fase=RAJADA" => Ok(1),
        _ => Err("Fase UART desconhecida".into()),
    }
}

fn validar_estimulo_host_rz025(bytes: &[u8]) -> Resultado<()> {
    let esperado = b"fase=UNICO submitted=55 quantidade=1\nfase=RAJADA submitted=31,32,33,34,35,36,37,38 quantidade=8\n";
    if bytes != esperado {
        return Err("Registro de bytes submetidos pelo host divergente".into());
    }
    Ok(())
}

fn linha_com_prefixo_completa(bytes: &[u8], prefixo: &str) -> bool {
    std::str::from_utf8(bytes).is_ok_and(|texto| {
        texto
            .split_inclusive('\n')
            .any(|linha| linha.ends_with('\n') && linha.starts_with(prefixo))
    })
}

fn linha_rx_quota_completa(bytes: &[u8], prefixo: &str) -> bool {
    std::str::from_utf8(bytes).is_ok_and(|texto| {
        texto.split_inclusive('\n').any(|linha| {
            if !linha.ends_with('\n') {
                return false;
            }
            let linha = linha.trim_end_matches(['\r', '\n']);
            let linha = linha.strip_prefix("raiz> ").unwrap_or(linha);
            linha.starts_with(prefixo)
        })
    })
}

fn validar_log_comparacao_vazia(bytes: &[u8]) -> Resultado<ResultadoComparacaoVazia> {
    let texto = std::str::from_utf8(bytes)?;
    let mut armadas: Vec<(usize, u64, u64)> = Vec::new();
    let mut resultados: Vec<(usize, ResultadoComparacaoVazia)> = Vec::new();
    for (posicao, linha_completa) in texto.split_inclusive('\n').enumerate() {
        if !linha_completa.ends_with('\n') && linha_completa.contains("[comparacao-vazia]") {
            return Err("Marcador de comparação incompleto".into());
        }
        let linha = linha_completa.trim_end_matches(['\r', '\n']);
        if linha.contains("[comparacao-vazia]") && !linha.starts_with("[comparacao-vazia]") {
            return Err("Eco ou prefixo antes do marcador de comparação".into());
        }
        if linha.starts_with("[comparacao-vazia]") {
            let tokens: Vec<_> = linha.split_whitespace().collect();
            match tokens.get(1).copied() {
                Some("ARMADA") => {
                    if tokens.len() != 5 {
                        return Err("Marcador ARMADA malformado".into());
                    }
                    let revisao = campo_unico_medicao(linha, "revisao=")?;
                    let janela = campo_decimal_medicao(linha, "janela_tiques=")?;
                    let inicio = campo_decimal_medicao(linha, "inicio=")?;
                    if revisao != "RZ-025" || janela != 100 {
                        return Err("ARMADA não identifica a revisão e a janela atuais".into());
                    }
                    armadas.push((posicao, inicio, janela));
                }
                Some("RESULTADO") => {
                    if tokens.len() != 9 {
                        return Err("Marcador RESULTADO malformado".into());
                    }
                    let revisao = campo_unico_medicao(linha, "revisao=")?;
                    let janela = campo_decimal_medicao(linha, "janela_tiques=")?;
                    if revisao != "RZ-025" || janela != 100 {
                        return Err("RESULTADO não identifica a revisão e a janela atuais".into());
                    }
                    resultados.push((
                        posicao,
                        ResultadoComparacaoVazia {
                            inicio: campo_decimal_medicao(linha, "inicio=")?,
                            fim: campo_decimal_medicao(linha, "fim=")?,
                            decorrido: campo_decimal_medicao(linha, "decorrido=")?,
                            consultas_ipc_vazias: campo_decimal_medicao(
                                linha,
                                "consultas_ipc_vazias=",
                            )?,
                            voltas_sem_trabalho: campo_decimal_medicao(
                                linha,
                                "voltas_sem_trabalho=",
                            )?,
                        },
                    ));
                }
                Some("FALHA_JANELA" | "FALHA_ESTADO") => {
                    return Err(format!("A instrumentação recusou a medição: {linha}").into());
                }
                _ => return Err(format!("Marcador desconhecido da comparação: {linha}").into()),
            }
        }
        if linha.contains("SERVICO_FALHOU") || linha.contains("panic") {
            return Err(format!("Falha de serviço/kernel durante a medição: {linha}").into());
        }
    }
    if armadas.len() != 1 || resultados.len() != 1 {
        return Err(format!(
            "Esperados ARMADA e RESULTADO únicos; recebidos {} e {}",
            armadas.len(),
            resultados.len()
        )
        .into());
    }
    let (pos_armada, inicio_armada, janela_armada) = armadas[0];
    let (pos_resultado, medicao) = resultados[0];
    if pos_resultado <= pos_armada
        || janela_armada != 100
        || medicao.inicio != inicio_armada
        || medicao.decorrido != 100
        || medicao.fim.wrapping_sub(medicao.inicio) != 100
    {
        return Err("Janela, tick inicial ou duração divergem da ARMADA de 100 tiques".into());
    }
    Ok(medicao)
}

fn campo_unico_medicao<'a>(linha: &'a str, nome: &str) -> Resultado<&'a str> {
    let mut valores = linha
        .split_whitespace()
        .filter_map(|campo| campo.strip_prefix(nome));
    let valor = valores
        .next()
        .ok_or_else(|| format!("Campo ausente: {nome}"))?;
    if valores.next().is_some() {
        return Err(format!("Campo duplicado: {nome}").into());
    }
    Ok(valor)
}

fn campo_decimal_medicao(linha: &str, nome: &str) -> Resultado<u64> {
    let valor = campo_unico_medicao(linha, nome)?;
    if valor.is_empty() || !valor.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(format!("Campo decimal malformado: {nome}").into());
    }
    Ok(valor.parse()?)
}

fn campo_decimal_uart(campo: &str, prefixo: &str) -> Resultado<u64> {
    let valor = campo.strip_prefix(prefixo).ok_or("Campo UART ausente")?;
    if valor.is_empty() || !valor.bytes().all(|b| b.is_ascii_digit()) {
        return Err("Campo decimal UART malformado".into());
    }
    Ok(valor.parse()?)
}

fn campo_hex_uart(campo: &str, prefixo: &str) -> Resultado<u8> {
    let valor = campo.strip_prefix(prefixo).ok_or("Campo UART ausente")?;
    let valor = valor
        .strip_prefix("0x")
        .ok_or("Campo hexadecimal UART malformado")?;
    if valor.is_empty() || valor.len() > 2 || !valor.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err("Campo hexadecimal UART malformado".into());
    }
    Ok(u8::from_str_radix(valor, 16)?)
}

fn executar_teste_uart(
    raiz: &Path,
    serial: File,
    erros: File,
    instrumentado: bool,
) -> Resultado<()> {
    let reserva = UdpSocket::bind("127.0.0.1:17777")?;
    let variante = if instrumentado { "teste-uart" } else { "" };
    let mut comando = preparar_qemu_em(raiz, false, true, variante)?;
    comando
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    drop(reserva);
    let mut processo = ProcessoQemu(comando.spawn()?);
    let mut entrada = processo.0.stdin.take().ok_or("stdin indisponivel")?;
    let saida = processo.0.stdout.take().ok_or("stdout indisponivel")?;
    let stderr = processo.0.stderr.take().ok_or("stderr indisponivel")?;
    let (emissor, receptor) = mpsc::channel();
    let leitores = [
        leitor_saida(saida, true, emissor.clone()),
        leitor_saida(stderr, false, emissor),
    ];
    let mut captura = Captura {
        eventos: receptor,
        serial: Vec::new(),
        erros: Vec::new(),
        arquivo_serial: serial,
        arquivo_erros: erros,
    };
    let resultado = (|| -> Resultado<()> {
        captura.aguardar(
            &mut processo,
            "[teste] TERMINAL_PRONTO",
            0,
            Duration::from_secs(30),
        )?;
        aguardar_rede_geracao(&mut captura, &mut processo, 0, 1)?;
        captura.aguardar(
            &mut processo,
            "[supervisor] SERVICO_PRONTO",
            0,
            Duration::from_secs(30),
        )?;
        if instrumentado {
            captura.aguardar(
                &mut processo,
                "[teste-uart] BLOQUEADO",
                0,
                Duration::from_secs(10),
            )?;
            captura.drenar()?;
            let antes_byte = captura.serial.len();
            entrada.write_all(b"s")?;
            entrada.flush()?;
            captura.aguardar(
                &mut processo,
                "[teste-uart] DESPERTA_IRQ",
                antes_byte,
                Duration::from_secs(10),
            )?;
            captura.aguardar(
                &mut processo,
                "[teste-uart] JANELA_REARME",
                antes_byte,
                Duration::from_secs(10),
            )?;
            captura.drenar()?;
            let antes_janela = captura.serial.len();
            entrada.write_all(b"o")?;
            entrada.flush()?;
            let antes_confirmacao = "[teste-uart] ANTES_CONFIRMACAO pendente=1";
            captura.aguardar_linha(
                &mut processo,
                antes_confirmacao,
                antes_janela,
                Duration::from_secs(10),
            )?;
            // Os marcadores podem chegar juntos em uma leitura do pipe; use o
            // fim da linha anterior, não o fim atual do buffer, para ordenar.
            let depois_confirmacao = antes_janela
                + fim_linha(&captura.serial[antes_janela..], antes_confirmacao)
                    .ok_or("marcador anterior a confirmacao ausente")?;
            captura.aguardar_linha(
                &mut processo,
                "[teste-uart] REARME_VERIFICADO pendente=1",
                depois_confirmacao,
                Duration::from_secs(10),
            )?;
            enviar_serial(&mut entrada, b"bre\n")?;
            captura.aguardar(
                &mut processo,
                "Terminal PID=1 CPL=3 / rede PID=2 / supervisor PID=3 / nucleo CPL=0",
                antes_byte,
                Duration::from_secs(10),
            )?;
        } else {
            conferir_eco_terminal(
                &mut entrada,
                &mut captura,
                &mut processo,
                "UART_ISOLADA_123456789",
            )?;
        }
        // Rajadas curtas preservam o limite do FIFO e exigem resposta exata.
        for texto in [
            "UART_RAJADA_0123456789_ABCDEFGHIJKLMNOPQRSTUVWXYZ",
            "UART_REARME_9876543210",
        ] {
            captura.drenar()?;
            let inicio = captura.serial.len();
            let linha = format!("eco {texto}\n");
            for bloco in linha.as_bytes().chunks(4) {
                entrada.write_all(bloco)?;
                entrada.flush()?;
                thread::sleep(Duration::from_millis(50));
            }
            captura.aguardar_linha(&mut processo, texto, inicio, Duration::from_secs(10))?;
        }
        let inicio = enviar_comando(&mut entrada, &mut captura, "sobre\n")?;
        captura.aguardar(
            &mut processo,
            "Terminal PID=1 CPL=3 / rede PID=2 / supervisor PID=3 / nucleo CPL=0",
            inicio,
            Duration::from_secs(10),
        )?;
        Ok(())
    })();
    processo.encerrar();
    for leitor in leitores {
        let _ = leitor.join();
    }
    captura.drenar()?;
    resultado?;
    let texto = String::from_utf8_lossy(&captura.serial);
    for falha in [
        "FALHA_NUCLEO",
        "SERVICO_FALHOU",
        "SERVICO_FINALIZADO",
        "IRQ4 indisponivel",
        "Falha ao confirmar IRQ4",
        "Falha na espera IRQ4",
        "Evento inesperado; polling serial",
        "Erros UART LSR=",
        "[teste-uart] FALHA",
        "panic",
        "PANIC",
    ] {
        if texto.contains(falha) {
            return Err(format!("Falha observada: {falha}").into());
        }
    }
    Ok(())
}

fn testar_irq(raiz: &Path) -> Resultado<()> {
    let instante = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_nanos();
    let pasta = raiz.join(format!("evidencias/teste-irq-{instante}"));
    fs::create_dir(&pasta)?;
    let resultado = executar_teste_irq(
        raiz,
        File::create(pasta.join("serial.log"))?,
        File::create(pasta.join("qemu-stderr.log"))?,
        "teste-irq",
    );
    fs::write(
        pasta.join("resultado.txt"),
        match &resultado {
            Ok(()) => "APROVADO".to_owned(),
            Err(e) => format!("FALHOU: {e}"),
        },
    )?;
    println!("Evidencias: {}", pasta.display());
    resultado
}

fn executar_teste_irq(raiz: &Path, serial: File, erros: File, variante: &str) -> Resultado<()> {
    let reserva = UdpSocket::bind("127.0.0.1:17777")?;
    let mut comando = preparar_qemu_em(raiz, false, true, variante)?;
    comando
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    drop(reserva);
    let mut processo = ProcessoQemu(comando.spawn()?);
    let mut entrada = processo.0.stdin.take().ok_or("stdin indisponivel")?;
    let saida = processo.0.stdout.take().ok_or("stdout indisponivel")?;
    let stderr = processo.0.stderr.take().ok_or("stderr indisponivel")?;
    let (emissor, receptor) = mpsc::channel();
    let leitores = [
        leitor_saida(saida, true, emissor.clone()),
        leitor_saida(stderr, false, emissor),
    ];
    let mut captura = Captura {
        eventos: receptor,
        serial: Vec::new(),
        erros: Vec::new(),
        arquivo_serial: serial,
        arquivo_erros: erros,
    };
    let resultado = (|| -> Resultado<()> {
        captura.aguardar(
            &mut processo,
            "[teste] TERMINAL_PRONTO",
            0,
            Duration::from_secs(30),
        )?;
        aguardar_rede_geracao(&mut captura, &mut processo, 0, 1)?;
        captura.aguardar(
            &mut processo,
            "[supervisor] SERVICO_PRONTO",
            0,
            Duration::from_secs(30),
        )?;
        let inicio = enviar_comando(&mut entrada, &mut captura, "teste-irq\n")?;
        for marcador in [
            "[teste-irq] SEM_AUTORIDADE CPL3",
            "[teste-irq] REDE_PRONTA_PARA_BLOQUEAR CPL3",
            "[teste-irq] ARMADA CPL3",
            "[teste-irq] BLOQUEADO",
        ] {
            captura.aguardar_linha(&mut processo, marcador, inicio, Duration::from_secs(30))?;
        }
        let armado = b"[teste-irq] ARMADA CPL3";
        let depois_armar = inicio
            + captura.serial[inicio..]
                .windows(armado.len())
                .position(|w| w == armado)
                .ok_or("marcador de armacao ausente")?
            + armado.len();
        captura.aguardar_linha(
            &mut processo,
            "[teste-irq] OCIOSO",
            depois_armar,
            Duration::from_secs(30),
        )?;
        let antes_a = captura.serial.len();
        enviar_serial(&mut entrada, b"A")?;
        captura.aguardar_linha(
            &mut processo,
            "[teste-irq] IRQ4_CPL0",
            antes_a,
            Duration::from_secs(30),
        )?;
        captura.aguardar_linha(
            &mut processo,
            "[teste-irq] JANELA CPL3",
            inicio,
            Duration::from_secs(30),
        )?;
        captura.aguardar(
            &mut processo,
            "[teste-irq] IPC_IMPOSTORA_REJEITADA seq=",
            inicio,
            Duration::from_secs(30),
        )?;
        enviar_serial(&mut entrada, b"B")?;
        captura.aguardar_linha(
            &mut processo,
            "[teste-irq] MASCARA_SEM_TEMPESTADE CPL3",
            inicio,
            Duration::from_secs(30),
        )?;
        captura.aguardar(
            &mut processo,
            "[teste-irq] IRQ_ORIGINAL_CONFIRMADA seq=",
            inicio,
            Duration::from_secs(30),
        )?;
        captura.aguardar_linha(
            &mut processo,
            "[teste-irq] REARMADA CPL3",
            inicio,
            Duration::from_secs(30),
        )?;
        // Procura o bloqueio posterior ao rearme, mesmo se os dois marcadores
        // chegaram na mesma leitura do pipe.
        let marcador = b"[teste-irq] REARMADA CPL3";
        let depois_rearme = inicio
            + captura.serial[inicio..]
                .windows(marcador.len())
                .position(|w| w == marcador)
                .ok_or("marcador de rearme ausente")?
            + marcador.len();
        captura.aguardar_linha(
            &mut processo,
            "[teste-irq] BLOQUEADO",
            depois_rearme,
            Duration::from_secs(30),
        )?;
        enviar_serial(&mut entrada, b"C")?;
        captura.aguardar_linha(
            &mut processo,
            "[teste-irq] ENTREGA_CONFIRMACAO_REARME CPL3",
            inicio,
            Duration::from_secs(30),
        )?;
        Ok(())
    })();
    processo.encerrar();
    for leitor in leitores {
        let _ = leitor.join();
    }
    captura.drenar()?;
    resultado?;
    let texto = String::from_utf8_lossy(&captura.serial);
    validar_log_teste_irq(&texto)?;
    for falha in [
        "[teste-irq] FALHA",
        "FALHA_NUCLEO",
        "SERVICO_FALHOU",
        "SERVICO_FINALIZADO",
        "panic",
        "PANIC",
    ] {
        if texto.contains(falha) {
            return Err(format!("Falha observada: {falha}").into());
        }
    }
    Ok(())
}

fn validar_log_teste_irq(log: &str) -> Resultado<()> {
    // Valor de contrato_sistema::classe_evento::IRQ (ABI 2).
    const CLASSE_EVENTO_IRQ: u64 = 3;
    let mut posicoes = [None; 7];
    let mut sequencia_irq = None;
    let mut sequencia_impostora = None;
    let mut sequencia_confirmada = None;
    let mut codigo_impostor = None;
    let mut retorno_impostor = None;
    let mut deslocamento = 0usize;
    for registro in log.split_inclusive('\n') {
        let Some(linha) = registro.strip_suffix('\n') else {
            if registro.starts_with("[teste-irq]") {
                return Err("Log IRQ terminou com marcador incompleto".into());
            }
            deslocamento += registro.len();
            continue;
        };
        let linha = linha.strip_suffix('\r').unwrap_or(linha);
        let marcador = if let Some(resto) = linha.strip_prefix("[teste-irq] IRQ_ENTREGUE seq=") {
            if linha.split_whitespace().count() != 3 {
                return Err("Marcador IRQ_ENTREGUE malformado".into());
            }
            sequencia_irq = Some(resto.parse::<u64>()?);
            Some(0)
        } else if let Some(resto) = linha.strip_prefix("[teste-irq] IPC_IMPOSTORA_REJEITADA seq=") {
            if !resto.ends_with(" CPL3") || linha.split_whitespace().count() != 6 {
                return Err("Marcador IPC impostora malformado".into());
            }
            sequencia_impostora = campo_numerico(linha, "seq");
            codigo_impostor = campo_numerico(linha, "codigo");
            retorno_impostor = campo_numerico(linha, "retorno");
            if codigo_impostor != Some(CLASSE_EVENTO_IRQ) || retorno_impostor != Some(u64::MAX - 1)
            {
                return Err("IPC impostora sem classe IRQ ou PERMISSAO".into());
            }
            Some(1)
        } else if let Some(resto) = linha.strip_prefix("[teste-irq] IRQ_ORIGINAL_CONFIRMADA seq=") {
            let Some(resto) = resto.strip_suffix(" CPL3") else {
                return Err("Marcador de confirmação sem CPL3".into());
            };
            if linha.split_whitespace().count() != 4 {
                return Err("Marcador de confirmação malformado".into());
            }
            sequencia_confirmada = Some(resto.parse::<u64>()?);
            Some(4)
        } else {
            match linha {
                "[teste-irq] JANELA CPL3" => Some(2),
                "[teste-irq] MASCARA_SEM_TEMPESTADE CPL3" => Some(3),
                "[teste-irq] REARMADA CPL3" => Some(5),
                "[teste-irq] ENTREGA_CONFIRMACAO_REARME CPL3" => Some(6),
                _ => None,
            }
        };
        if let Some(indice) = marcador {
            if posicoes[indice].replace(deslocamento).is_some() {
                return Err("Marcador duplicado no ensaio IRQ".into());
            }
        }
        deslocamento += registro.len();
    }
    if posicoes.iter().any(Option::is_none) {
        return Err("Faltam marcadores do ensaio IRQ".into());
    }
    if sequencia_irq.is_none_or(|sequencia| sequencia == 0)
        || sequencia_irq != sequencia_impostora
        || sequencia_irq != sequencia_confirmada
        || codigo_impostor.is_none()
        || retorno_impostor.is_none()
    {
        return Err("Sequência impostora/confirmada não corresponde à IRQ entregue".into());
    }
    if !(posicoes[0] < posicoes[1]
        && posicoes[1] < posicoes[2]
        && posicoes[2] < posicoes[3]
        && posicoes[3] < posicoes[4]
        && posicoes[4] < posicoes[5]
        && posicoes[5] < posicoes[6])
    {
        return Err("Ordem de IRQ, IPC impostora, confirmação e rearme inválida".into());
    }
    Ok(())
}

fn testar_cancelamento_aplicacao(raiz: &Path) -> Resultado<()> {
    let instante = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_nanos();
    let pasta = raiz.join(format!(
        "evidencias/teste-cancelamento-aplicacao-{instante}"
    ));
    fs::create_dir(&pasta)?;
    let resultado = executar_teste_cancelamento_aplicacao(
        raiz,
        File::create(pasta.join("serial.log"))?,
        File::create(pasta.join("qemu-stderr.log"))?,
    );
    let relatorio = match &resultado {
        Ok(()) => "# Ensaio T03-24 — cancelamento local após envio aceito\n\nImagem opt-in; QEMU/TCG, pc/i440fx, 1 CPU, 256 MiB, OVMF e e1000.\n\nResultado: **APROVADO**. O cliente obteve aceitação IPC para dois IDs e recebeu confirmação de que o serviço de rede recebeu ambos. Cancelou localmente o primeiro, manteve o segundo ativo e liberou respostas em ordem inversa à dos pedidos: primeiro a resposta correlacionada ao ativo, depois a resposta tardia do cancelado. A primeira concluiu a operação ativa; a tardia foi correlacionada e ignorada, mantendo o estado local cancelado.\n\nLimite: o cancelamento demonstrado pertence à política local do cliente. O protocolo de teste não pede ao servidor para desfazer ou compensar efeitos, nem demonstra rollback, idempotência ou comportamento de outros clientes. Logs: `serial.log` e `qemu-stderr.log`.\n".to_owned(),
        Err(erro) => format!(
            "# Ensaio T03-24 — cancelamento local após envio aceito\n\nImagem opt-in; QEMU/TCG, pc/i440fx, 1 CPU, 256 MiB, OVMF e e1000.\n\nResultado: **FALHOU**. `{erro}`\n\nO cenário não reivindica reversão de efeito no serviço. Logs: `serial.log` e `qemu-stderr.log`.\n"
        ),
    };
    fs::write(pasta.join("resultado.txt"), relatorio)?;
    println!("Evidências: {}", pasta.display());
    resultado
}

fn testar_falha_preparacao(raiz: &Path) -> Resultado<()> {
    let instante = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_nanos();
    let pasta = raiz.join(format!("evidencias/teste-falha-preparacao-{instante}"));
    fs::create_dir(&pasta)?;
    let serial = File::create(pasta.join("serial.log"))?;
    let erros = File::create(pasta.join("qemu-stderr.log"))?;
    let resultado = executar_teste_falha_preparacao(raiz, serial, erros);
    let relatorio = match &resultado {
        Ok(()) => "# Ensaio T03-10 — falha de preparação da rede\n\nImagem opt-in; QEMU/TCG, pc/i440fx, 1 CPU, 256 MiB, OVMF e e1000.\n\nResultado: **APROVADO**. O comando autorizado do supervisor entrou no reinício real. A fixture passou um ELF inválido ao loader real da arena ainda não publicada; o loader recusou a preparação antes da revogação/commit. O erro `IMAGEM` retornou ao terminal. A referência da rede permaneceu na geração 1 e a rede anterior respondeu ao ping do gateway e a ecos UDP após a recusa.\n\nLimite: a falha é injetada na validação de ELF do processo de preparação; não simula esgotamento físico, falha de página nem toda classe de falha do loader. Não demonstra recuperação de conexão ou estado de aplicação. Logs: `serial.log` e `qemu-stderr.log`.\n".to_owned(),
        Err(erro) => format!("# Ensaio T03-10 — falha de preparação da rede\n\nImagem opt-in; QEMU/TCG, pc/i440fx, 1 CPU, 256 MiB, OVMF e e1000.\n\nResultado: **FALHOU**. `{erro}`\n\nLogs: `serial.log` e `qemu-stderr.log`.\n"),
    };
    fs::write(pasta.join("resultado.txt"), relatorio)?;
    println!("Evidências: {}", pasta.display());
    resultado
}

fn executar_teste_falha_preparacao(raiz: &Path, serial: File, erros: File) -> Resultado<()> {
    let reserva = UdpSocket::bind("127.0.0.1:17777")?;
    let mut comando = preparar_qemu_em(raiz, false, true, "teste-falha-preparacao")?;
    comando
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    drop(reserva);
    let mut processo = ProcessoQemu(comando.spawn()?);
    let mut entrada = processo.0.stdin.take().ok_or("stdin indisponível")?;
    let saida = processo.0.stdout.take().ok_or("stdout indisponível")?;
    let stderr = processo.0.stderr.take().ok_or("stderr indisponível")?;
    let (emissor, receptor) = mpsc::channel();
    let leitores = [
        leitor_saida(saida, true, emissor.clone()),
        leitor_saida(stderr, false, emissor),
    ];
    let mut captura = Captura {
        eventos: receptor,
        serial: Vec::new(),
        erros: Vec::new(),
        arquivo_serial: serial,
        arquivo_erros: erros,
    };
    let resultado = (|| -> Resultado<()> {
        captura.aguardar(
            &mut processo,
            "[teste] TERMINAL_PRONTO",
            0,
            Duration::from_secs(30),
        )?;
        aguardar_rede_geracao(&mut captura, &mut processo, 0, 1)?;
        captura.aguardar(
            &mut processo,
            "[supervisor] SERVICO_PRONTO",
            0,
            Duration::from_secs(30),
        )?;

        let mut evidencias = EvidenciasSmoke::default();
        conferir_conectividade(
            &mut entrada,
            &mut captura,
            &mut processo,
            1,
            "antes da falha",
            &mut evidencias,
        )?;
        let inicio = enviar_comando(&mut entrada, &mut captura, "reiniciar-rede\n")?;
        for marcador in [
            "[teste-falha-preparacao] ERRO_REAL_LOADER antiga=0x100000002 nova=0x200000002 motivo=ELF_INVALIDO",
            "[microkernel] REINICIO_RECUSADO referencia=0x100000002 motivo=ELF de servico invalido",
            "[supervisor] OPERACAO_RECUSADA codigo=0xfffffffffffffff7 operacao=9 geracao=1",
        ] {
            captura.aguardar_condicao(
                &mut processo,
                marcador,
                inicio,
                Duration::from_secs(10),
                |bytes| linha_falha_preparacao_presente(bytes, marcador),
            )?;
        }
        captura.drenar()?;
        validar_log_falha_preparacao(&captura.serial)?;
        conferir_instancia(&mut entrada, &mut captura, &mut processo, 1, 1)?;
        conferir_conectividade(
            &mut entrada,
            &mut captura,
            &mut processo,
            1,
            "após falha de preparação",
            &mut evidencias,
        )?;
        Ok(())
    })();
    processo.encerrar();
    for leitor in leitores {
        let _ = leitor.join();
    }
    captura.drenar()?;
    resultado
}

fn validar_log_falha_preparacao(serial: &[u8]) -> Resultado<()> {
    let serial = String::from_utf8_lossy(serial);
    let mut linhas = Vec::new();
    for registro in serial.split_inclusive('\n') {
        let Some(linha) = registro.strip_suffix('\n') else {
            let fragmento = registro.strip_prefix("raiz> ").unwrap_or(registro);
            if [
                "[teste-falha-preparacao]",
                "[microkernel] REINICIO_RECUSADO",
                "[supervisor] OPERACAO_RECUSADA",
            ]
            .iter()
            .any(|prefixo| fragmento.starts_with(prefixo))
            {
                return Err("Log terminou com marcador de falha de preparação incompleto".into());
            }
            continue;
        };
        linhas.push(normalizar_prompt_falha_preparacao(linha));
    }
    let linhas: Vec<&str> = linhas
        .into_iter()
        .map(|linha| linha.strip_suffix('\r').unwrap_or(linha))
        .collect();
    let marcadores = [
        "[teste-falha-preparacao] ERRO_REAL_LOADER antiga=0x100000002 nova=0x200000002 motivo=ELF_INVALIDO",
        "[microkernel] REINICIO_RECUSADO referencia=0x100000002 motivo=ELF de servico invalido",
        "[supervisor] OPERACAO_RECUSADA codigo=0xfffffffffffffff7 operacao=9 geracao=1",
    ];
    let mut posicoes = [0usize; 3];
    for linha in &linhas {
        if linha.starts_with("[teste-falha-preparacao]") {
            if *linha != marcadores[0] || posicoes[0] != 0 {
                return Err(format!(
                    "Marcador da fixture ausente, duplicado ou malformado: {linha}"
                )
                .into());
            }
            posicoes[0] = linhas.iter().position(|item| item == linha).unwrap_or(0) + 1;
        } else if linha.starts_with("[microkernel] REINICIO_RECUSADO") {
            if *linha != marcadores[1] || posicoes[1] != 0 {
                return Err(
                    format!("Recusa do loader ausente, duplicada ou malformada: {linha}").into(),
                );
            }
            posicoes[1] = linhas.iter().position(|item| item == linha).unwrap_or(0) + 1;
        } else if linha.starts_with("[supervisor] OPERACAO_RECUSADA") {
            if *linha != marcadores[2] || posicoes[2] != 0 {
                return Err(format!(
                    "Erro do supervisor ausente, duplicado ou malformado: {linha}"
                )
                .into());
            }
            posicoes[2] = linhas.iter().position(|item| item == linha).unwrap_or(0) + 1;
        }
    }
    if posicoes.contains(&0) || !(posicoes[0] < posicoes[1] && posicoes[1] < posicoes[2]) {
        return Err(
            "Falha de preparação não correlacionou loader, kernel e supervisor em ordem".into(),
        );
    }
    if linhas
        .iter()
        .any(|linha| linha.starts_with("[microkernel] SERVICO_REINICIADO id=2"))
    {
        return Err("A instância antiga foi substituída apesar da falha de preparação".into());
    }
    Ok(())
}

fn normalizar_prompt_falha_preparacao(linha: &str) -> &str {
    let Some(depois_prompt) = linha.strip_prefix("raiz> ") else {
        return linha;
    };
    if depois_prompt.starts_with("[teste-falha-preparacao]")
        || depois_prompt.starts_with("[microkernel] REINICIO_RECUSADO")
        || depois_prompt.starts_with("[supervisor] OPERACAO_RECUSADA")
    {
        depois_prompt
    } else {
        linha
    }
}

fn linha_falha_preparacao_presente(serial: &[u8], esperada: &str) -> bool {
    String::from_utf8_lossy(serial)
        .split_inclusive('\n')
        .filter(|registro| registro.ends_with('\n'))
        .map(|registro| registro.trim_end_matches(['\r', '\n']))
        .any(|linha| normalizar_prompt_falha_preparacao(linha) == esperada)
}

fn executar_teste_cancelamento_aplicacao(raiz: &Path, serial: File, erros: File) -> Resultado<()> {
    let reserva = UdpSocket::bind("127.0.0.1:17777")?;
    let mut comando = preparar_qemu_em(raiz, false, true, "teste-cancelamento-aplicacao")?;
    comando
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    drop(reserva);
    let mut processo = ProcessoQemu(comando.spawn()?);
    let mut entrada = processo.0.stdin.take().ok_or("stdin indisponivel")?;
    let saida = processo.0.stdout.take().ok_or("stdout indisponivel")?;
    let stderr = processo.0.stderr.take().ok_or("stderr indisponivel")?;
    let (emissor, receptor) = mpsc::channel();
    let leitores = [
        leitor_saida(saida, true, emissor.clone()),
        leitor_saida(stderr, false, emissor),
    ];
    let mut captura = Captura {
        eventos: receptor,
        serial: Vec::new(),
        erros: Vec::new(),
        arquivo_serial: serial,
        arquivo_erros: erros,
    };
    let resultado = (|| -> Resultado<()> {
        captura.aguardar(
            &mut processo,
            "[teste] TERMINAL_PRONTO",
            0,
            Duration::from_secs(30),
        )?;
        aguardar_rede_geracao(&mut captura, &mut processo, 0, 1)?;
        captura.aguardar(
            &mut processo,
            "[supervisor] SERVICO_PRONTO",
            0,
            Duration::from_secs(30),
        )?;
        let inicio = enviar_comando(&mut entrada, &mut captura, "teste-cancelamento-aplicacao\n")?;
        for marcador in [
            "[teste-cancelamento] ENVIO_ACEITO id=0x2401",
            "[teste-cancelamento] ENVIO_ACEITO id=0x2402",
            "[teste-cancelamento] SERVIDOR_RECEBEU ids=0x2401,0x2402 CPL3",
            "[teste-cancelamento] CANCELADO_LOCAL id=0x2401",
            "[teste-cancelamento] RESPOSTA_ATIVA id=0x2402 valor=0x2222",
            "[teste-cancelamento] RESPOSTA_TARDIA_IGNORADA id=0x2401 estado=cancelada",
            "[teste-cancelamento] CANCELAMENTO_PRESERVADO",
        ] {
            captura.aguardar_linha(&mut processo, marcador, inicio, Duration::from_secs(10))?;
        }
        captura.drenar()?;
        let log = String::from_utf8_lossy(&captura.serial);
        validar_log_cancelamento_aplicacao(&log)?;
        for falha in [
            "[teste-cancelamento] FALHA",
            "SERVICO_FALHOU",
            "SERVICO_FINALIZADO",
            "panic",
            "PANIC",
        ] {
            if log.contains(falha) {
                return Err(format!("Falha no ensaio de cancelamento: {falha}").into());
            }
        }
        Ok(())
    })();
    processo.encerrar();
    for leitor in leitores {
        let _ = leitor.join();
    }
    captura.drenar()?;
    resultado
}

fn validar_log_cancelamento_aplicacao(log: &str) -> Resultado<()> {
    let esperados = [
        "[teste-cancelamento] ENVIO_ACEITO id=0x2401",
        "[teste-cancelamento] ENVIO_ACEITO id=0x2402",
        "[teste-cancelamento] SERVIDOR_RECEBEU ids=0x2401,0x2402 CPL3",
        "[teste-cancelamento] CANCELADO_LOCAL id=0x2401",
        "[teste-cancelamento] RESPOSTA_ATIVA id=0x2402 valor=0x2222",
        "[teste-cancelamento] RESPOSTA_TARDIA_IGNORADA id=0x2401 estado=cancelada",
        "[teste-cancelamento] CANCELAMENTO_PRESERVADO",
    ];
    let mut posicoes = [None; 7];
    let mut deslocamento = 0usize;
    for registro in log.split_inclusive('\n') {
        let Some(linha) = registro.strip_suffix('\n') else {
            if registro.starts_with("[teste-cancelamento]") {
                return Err("Log de cancelamento terminou com marcador incompleto".into());
            }
            deslocamento += registro.len();
            continue;
        };
        let linha = linha.strip_suffix('\r').unwrap_or(linha);
        if linha.starts_with("[teste-cancelamento] FALHA") {
            return Err(format!("Marcador de falha no ensaio: {linha}").into());
        }
        if linha.starts_with("[teste-cancelamento]") {
            let Some(indice) = esperados.iter().position(|esperado| linha == *esperado) else {
                return Err(format!("Marcador desconhecido ou malformado: {linha}").into());
            };
            if posicoes[indice].replace(deslocamento).is_some() {
                return Err(format!("Marcador duplicado: {}", esperados[indice]).into());
            }
        }
        deslocamento += registro.len();
    }
    if posicoes.iter().any(Option::is_none) {
        return Err("Faltam marcadores completos do ensaio de cancelamento".into());
    }
    if !(posicoes[0] < posicoes[1]
        && posicoes[1] < posicoes[2]
        && posicoes[2] < posicoes[3]
        && posicoes[3] < posicoes[4]
        && posicoes[4] < posicoes[5]
        && posicoes[5] < posicoes[6])
    {
        return Err("Ordem dos pedidos, cancelamento e respostas está inválida".into());
    }
    Ok(())
}

fn testar_quota(raiz: &Path) -> Resultado<()> {
    let instante = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_nanos();
    let pasta = raiz.join(format!("evidencias/teste-quota-{instante}"));
    fs::create_dir(&pasta)?;
    let resultado = executar_teste_quota(
        raiz,
        File::create(pasta.join("serial.log"))?,
        File::create(pasta.join("qemu-stderr.log"))?,
    );
    fs::write(
        pasta.join("resultado.txt"),
        match &resultado {
            Ok(()) => "APROVADO".to_owned(),
            Err(e) => format!("FALHOU: {e}"),
        },
    )?;
    println!("Evidencias: {}", pasta.display());
    resultado
}

fn executar_teste_quota(raiz: &Path, serial: File, erros: File) -> Resultado<()> {
    let reserva = UdpSocket::bind("127.0.0.1:17777")?;
    let mut comando = preparar_qemu_em(raiz, false, true, "teste-quota")?;
    comando
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    drop(reserva);
    let mut processo = ProcessoQemu(comando.spawn()?);
    let mut entrada = processo.0.stdin.take().ok_or("stdin indisponivel")?;
    let saida = processo.0.stdout.take().ok_or("stdout indisponivel")?;
    let stderr = processo.0.stderr.take().ok_or("stderr indisponivel")?;
    let (emissor, receptor) = mpsc::channel();
    let leitores = [
        leitor_saida(saida, true, emissor.clone()),
        leitor_saida(stderr, false, emissor),
    ];
    let mut captura = Captura {
        eventos: receptor,
        serial: Vec::new(),
        erros: Vec::new(),
        arquivo_serial: serial,
        arquivo_erros: erros,
    };
    let resultado = (|| -> Resultado<()> {
        captura.aguardar(
            &mut processo,
            "[teste] TERMINAL_PRONTO",
            0,
            Duration::from_secs(30),
        )?;
        aguardar_rede_geracao(&mut captura, &mut processo, 0, 1)?;
        captura.aguardar(
            &mut processo,
            "[supervisor] SERVICO_PRONTO",
            0,
            Duration::from_secs(30),
        )?;
        let inicio = enviar_comando(&mut entrada, &mut captura, "teste-quota\n")?;
        captura.aguardar_linha(
            &mut processo,
            "[teste-quota] FIFO_RESERVA_LIBERACAO CPL3",
            inicio,
            Duration::from_secs(30),
        )?;
        Ok(())
    })();
    processo.encerrar();
    for leitor in leitores {
        let _ = leitor.join();
    }
    captura.drenar()?;
    resultado?;
    let texto = String::from_utf8_lossy(&captura.serial);
    for falha in [
        "[teste-quota] FALHA",
        "FALHA_NUCLEO",
        "SERVICO_FALHOU",
        "SERVICO_FINALIZADO",
        "panic",
        "PANIC",
    ] {
        if texto.contains(falha) {
            return Err(format!("Falha observada: {falha}").into());
        }
    }
    Ok(())
}

fn testar_vida(raiz: &Path) -> Resultado<()> {
    let instante = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_nanos();
    let pasta = raiz.join(format!("evidencias/teste-vida-{instante}"));
    fs::create_dir(&pasta)?;
    let resultado = executar_teste_vida(
        raiz,
        File::create(pasta.join("serial.log"))?,
        File::create(pasta.join("qemu-stderr.log"))?,
    );
    fs::write(
        pasta.join("resultado.txt"),
        match &resultado {
            Ok(()) => "APROVADO".to_owned(),
            Err(e) => format!("FALHOU: {e}"),
        },
    )?;
    println!("Evidencias: {}", pasta.display());
    resultado
}

fn executar_teste_vida(raiz: &Path, serial: File, erros: File) -> Resultado<()> {
    let reserva = UdpSocket::bind("127.0.0.1:17777")?;
    let mut comando = preparar_qemu_em(raiz, false, true, "teste-vida")?;
    comando
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    drop(reserva);
    let mut processo = ProcessoQemu(comando.spawn()?);
    let mut entrada = processo.0.stdin.take().ok_or("stdin indisponivel")?;
    let saida = processo.0.stdout.take().ok_or("stdout indisponivel")?;
    let stderr = processo.0.stderr.take().ok_or("stderr indisponivel")?;
    let (emissor, receptor) = mpsc::channel();
    let leitores = [
        leitor_saida(saida, true, emissor.clone()),
        leitor_saida(stderr, false, emissor),
    ];
    let mut captura = Captura {
        eventos: receptor,
        serial: Vec::new(),
        erros: Vec::new(),
        arquivo_serial: serial,
        arquivo_erros: erros,
    };
    let resultado = (|| -> Resultado<()> {
        captura.aguardar(
            &mut processo,
            "[teste] TERMINAL_PRONTO",
            0,
            Duration::from_secs(30),
        )?;
        aguardar_rede_geracao(&mut captura, &mut processo, 0, 1)?;
        let inicio = enviar_comando(&mut entrada, &mut captura, "teste-vida-permissao\n")?;
        captura.aguardar_linha(
            &mut processo,
            "[teste-vida-servico] PERMISSAO_NEGADA CPL3",
            inicio,
            Duration::from_secs(10),
        )?;
        let inicio = enviar_comando(&mut entrada, &mut captura, "teste-vida-iniciar\n")?;
        captura.aguardar_linha(
            &mut processo,
            "[teste-vida-servico] ETAPA 1 CPL3",
            inicio,
            Duration::from_secs(10),
        )?;
        let fila = enviar_comando(&mut entrada, &mut captura, "teste-vida-encher\n")?;
        captura.aguardar_linha(
            &mut processo,
            "[teste-vida-servico] FILA_CHEIA CONFIRMADA",
            fila,
            Duration::from_secs(10),
        )?;
        let falha = enviar_comando(&mut entrada, &mut captura, "falhar-rede\n")?;
        aguardar_evento(
            &mut captura,
            &mut processo,
            falha,
            "[microkernel] SERVICO_FALHOU",
            &["id=2", "vetor=6", "referencia=0x100000002"],
        )?;
        captura.aguardar_linha(
            &mut processo,
            "[teste-vida-servico] ETAPA 3 CPL3",
            falha,
            Duration::from_secs(30),
        )?;
        captura.aguardar_linha(
            &mut processo,
            "[teste-vida] BLOQUEADO",
            inicio,
            Duration::from_secs(10),
        )?;
        aguardar_evento(
            &mut captura,
            &mut processo,
            inicio,
            "[microkernel] SERVICO_ENCERRADO",
            &["id=2", "referencia=0x200000002"],
        )?;
        aguardar_rede_geracao(&mut captura, &mut processo, falha, 3)?;
        let falha2 = enviar_comando(&mut entrada, &mut captura, "falhar-rede\n")?;
        aguardar_evento(
            &mut captura,
            &mut processo,
            falha2,
            "[microkernel] SERVICO_FALHOU",
            &["id=2", "vetor=6", "referencia=0x300000002"],
        )?;
        captura.aguardar_linha(
            &mut processo,
            "[teste-vida-servico] ETAPA 5 CPL3",
            falha2,
            Duration::from_secs(30),
        )?;
        for n in 1..5 {
            conferir(
                linhas_ordenadas(
                    &captura.serial[inicio..],
                    &format!("[teste-vida-servico] ETAPA {n} CPL3"),
                    &format!("[teste-vida-servico] ETAPA {} CPL3", n + 1),
                ),
                "Etapas de vida fora de ordem",
            )?;
        }
        aguardar_rede_geracao(&mut captura, &mut processo, falha, 4)?;
        conferir_instancia(&mut entrada, &mut captura, &mut processo, 1, 4)?;
        conferir_conectividade(
            &mut entrada,
            &mut captura,
            &mut processo,
            4,
            "apos teste de notificacoes",
            &mut EvidenciasSmoke::default(),
        )?;
        Ok(())
    })();
    processo.encerrar();
    for leitor in leitores {
        let _ = leitor.join();
    }
    captura.drenar()?;
    resultado?;
    let texto = String::from_utf8_lossy(&captura.serial);
    if texto.matches("[microkernel] SERVICO_FALHOU").count() != 2 {
        return Err("Numero inesperado de falhas de servico".into());
    }
    if texto.matches("[microkernel] SERVICO_ENCERRADO").count() != 1 {
        return Err("Numero inesperado de encerramentos".into());
    }
    for falha in [
        "FALHA_NUCLEO",
        "SERVICO_FINALIZADO",
        "[teste-vida-servico] FALHA",
        "panic",
        "PANIC",
    ] {
        if texto.contains(falha) {
            return Err(format!("Falha observada: {falha}").into());
        }
    }
    Ok(())
}

fn testar_observacao(raiz: &Path) -> Resultado<()> {
    let instante = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_nanos();
    let pasta = raiz.join(format!("evidencias/teste-observacao-{instante}"));
    fs::create_dir(&pasta)?;
    let resultado = executar_teste_observacao(
        raiz,
        File::create(pasta.join("serial.log"))?,
        File::create(pasta.join("qemu-stderr.log"))?,
    );
    fs::write(
        pasta.join("resultado.txt"),
        match &resultado {
            Ok(()) => "APROVADO".to_owned(),
            Err(e) => format!("FALHOU: {e}"),
        },
    )?;
    println!("Evidencias: {}", pasta.display());
    resultado
}

fn executar_teste_observacao(raiz: &Path, serial: File, erros: File) -> Resultado<()> {
    let reserva = UdpSocket::bind("127.0.0.1:17777")?;
    let mut comando = preparar_qemu_em(raiz, false, true, "teste-observacao")?;
    comando
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    drop(reserva);
    let mut processo = ProcessoQemu(comando.spawn()?);
    let mut entrada = processo.0.stdin.take().ok_or("stdin indisponivel")?;
    let saida = processo.0.stdout.take().ok_or("stdout indisponivel")?;
    let stderr = processo.0.stderr.take().ok_or("stderr indisponivel")?;
    let (emissor, receptor) = mpsc::channel();
    let leitores = [
        leitor_saida(saida, true, emissor.clone()),
        leitor_saida(stderr, false, emissor),
    ];
    let mut captura = Captura {
        eventos: receptor,
        serial: Vec::new(),
        erros: Vec::new(),
        arquivo_serial: serial,
        arquivo_erros: erros,
    };
    let resultado = (|| -> Resultado<()> {
        captura.aguardar(
            &mut processo,
            "[teste] TERMINAL_PRONTO",
            0,
            Duration::from_secs(30),
        )?;
        aguardar_rede_geracao(&mut captura, &mut processo, 0, 1)?;
        let inicio = enviar_comando(&mut entrada, &mut captura, "teste-observar-negado\n")?;
        captura.aguardar_linha(
            &mut processo,
            "[teste-observacao] PERMISSAO_NEGADA CPL3",
            inicio,
            Duration::from_secs(10),
        )?;
        let inicio = enviar_comando(&mut entrada, &mut captura, "teste-observar\n")?;
        captura.aguardar_linha(
            &mut processo,
            "[teste-ciclo-espera] OBSERVANDO referencia=0x100000002",
            inicio,
            Duration::from_secs(10),
        )?;
        let inicio = enviar_comando(&mut entrada, &mut captura, "teste-falhar-observado\n")?;
        aguardar_evento(
            &mut captura,
            &mut processo,
            inicio,
            "[microkernel] SERVICO_FALHOU",
            &["id=2", "vetor=6"],
        )?;
        captura.aguardar_linha(
            &mut processo,
            "[teste-observacao] SUPERVISOR_ALVO_INDISPONIVEL CPL3",
            inicio,
            Duration::from_secs(10),
        )?;
        conferir(
            linhas_ordenadas(
                &captura.serial[inicio..],
                "[teste-ciclo-espera] ALVO_INDISPONIVEL referencia=0x100000002",
                "[teste-observacao] SUPERVISOR_ALVO_INDISPONIVEL CPL3",
            ),
            "Resposta do supervisor sem invalidação anterior",
        )?;
        let inicio = enviar_comando(&mut entrada, &mut captura, "teste-referencia-observada 0\n")?;
        captura.aguardar_linha(
            &mut processo,
            "[teste-observacao] REFERENCIA etapa=0 CPL3",
            inicio,
            Duration::from_secs(10),
        )?;
        let inicio = enviar_comando(&mut entrada, &mut captura, "reiniciar-rede\n")?;
        aguardar_reinicio(&mut captura, &mut processo, inicio, 1, 2, false)?;
        let inicio = enviar_comando(&mut entrada, &mut captura, "teste-referencia-observada 1\n")?;
        captura.aguardar_linha(
            &mut processo,
            "[teste-observacao] REFERENCIA etapa=1 CPL3",
            inicio,
            Duration::from_secs(10),
        )?;
        conferir_instancia(&mut entrada, &mut captura, &mut processo, 1, 2)?;
        conferir_conectividade(
            &mut entrada,
            &mut captura,
            &mut processo,
            2,
            "apos falha observada",
            &mut EvidenciasSmoke::default(),
        )?;
        Ok(())
    })();
    processo.encerrar();
    for leitor in leitores {
        let _ = leitor.join();
    }
    captura.drenar()?;
    resultado?;
    let texto = String::from_utf8_lossy(&captura.serial);
    if texto.matches("[microkernel] SERVICO_FALHOU").count() != 1 {
        return Err("Numero inesperado de falhas de servico".into());
    }
    for falha in [
        "FALHA_NUCLEO",
        "SERVICO_FINALIZADO",
        "[teste-observacao] FALHA",
        "panic",
        "PANIC",
    ] {
        if texto.contains(falha) {
            return Err(format!("Falha observada: {falha}").into());
        }
    }
    Ok(())
}

fn prazo_ciclo(serial: &[u8], referencia: u64) -> Option<u64> {
    let prefixo = format!("[teste-ciclo-espera] BLOQUEADA referencia={referencia:#x} prazo=");
    String::from_utf8_lossy(serial)
        .split_inclusive('\n')
        .find_map(|linha| {
            if !linha.ends_with('\n') {
                return None;
            }
            let linha = linha.trim_end_matches(['\r', '\n']);
            let linha = linha.strip_prefix("raiz> ").unwrap_or(linha);
            linha.strip_prefix(&prefixo)?.parse().ok()
        })
}

fn prazo_ciclo_apos_driver(serial: &[u8], referencia: u64) -> Option<u64> {
    let prefixo = format!("[teste-ciclo-espera] DRIVER_ATIVO referencia={referencia:#x}");
    let inicio_driver = posicao_marcador_ciclo(serial, &prefixo)?;
    prazo_ciclo(&serial[inicio_driver..], referencia)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct SnapshotDriverCicloHost {
    referencia: u64,
    mmio: u64,
    arena_fisica: u64,
    indice_rx: u64,
    indice_tx: u64,
    recebidos: u64,
    transmitidos: u64,
}

fn snapshot_driver_ciclo(
    serial: &[u8],
    marcador: &str,
    referencia: u64,
) -> Option<SnapshotDriverCicloHost> {
    let prefixo = format!("[teste-ciclo-espera] {marcador} ");
    let mut resultado = None;
    for registro in String::from_utf8_lossy(serial).split_inclusive('\n') {
        if !registro.ends_with('\n') {
            continue;
        }
        let linha = registro.trim_end_matches(['\r', '\n']);
        let linha = linha.strip_prefix("raiz> ").unwrap_or(linha);
        if !linha.starts_with(&prefixo) {
            continue;
        }
        if resultado.is_some() {
            return None;
        }
        let campos: Vec<_> = linha.split_ascii_whitespace().collect();
        let preservado = marcador == "DRIVER_PRESERVADO";
        let esperado = if preservado { 11 } else { 10 };
        if campos.len() != esperado
            || campos[0] != "[teste-ciclo-espera]"
            || campos[1] != marcador
            || campos[2] != format!("referencia={referencia:#x}")
            || campos[9] != if preservado { "igual=1" } else { "CPL3" }
            || preservado && campos[10] != "CPL3"
        {
            return None;
        }
        let numero = |indice: usize, nome: &str, radix: u32| -> Option<u64> {
            let valor = campos.get(indice)?.strip_prefix(nome)?;
            if radix == 16 {
                u64::from_str_radix(valor.strip_prefix("0x")?, 16).ok()
            } else {
                valor.parse().ok()
            }
        };
        let snapshot = SnapshotDriverCicloHost {
            referencia,
            mmio: numero(3, "mmio=", 16)?,
            arena_fisica: numero(4, "arena=", 16)?,
            indice_rx: numero(5, "rx_idx=", 10)?,
            indice_tx: numero(6, "tx_idx=", 10)?,
            recebidos: numero(7, "rx=", 10)?,
            transmitidos: numero(8, "tx=", 10)?,
        };
        if snapshot.mmio == 0
            || snapshot.arena_fisica == 0
            || snapshot.indice_rx >= 32
            || snapshot.indice_tx >= 32
        {
            return None;
        }
        resultado = Some(snapshot);
    }
    resultado
}

fn posicao_marcador_ciclo(serial: &[u8], prefixo: &str) -> Option<usize> {
    let mut deslocamento = 0usize;
    let mut resultado = None;
    for registro in serial.split_inclusive(|b| *b == b'\n') {
        if registro.ends_with(b"\n") {
            let linha = String::from_utf8_lossy(registro);
            let linha = linha.trim_end_matches(['\r', '\n']);
            let linha = linha.strip_prefix("raiz> ").unwrap_or(linha);
            if linha.starts_with(prefixo) {
                if resultado.is_some() {
                    return None;
                }
                resultado = Some(deslocamento);
            }
        }
        deslocamento += registro.len();
    }
    resultado
}

fn posicao_marcador_ciclo_entre(
    serial: &[u8],
    prefixo: &str,
    inicio: usize,
    fim: usize,
) -> Option<usize> {
    let mut deslocamento = 0usize;
    let mut resultado = None;
    for registro in serial.split_inclusive(|b| *b == b'\n') {
        if registro.ends_with(b"\n") {
            let linha = String::from_utf8_lossy(registro);
            let linha = linha.trim_end_matches(['\r', '\n']);
            let linha = linha.strip_prefix("raiz> ").unwrap_or(linha);
            if deslocamento >= inicio && deslocamento < fim && linha.starts_with(prefixo) {
                if resultado.is_some() {
                    return None;
                }
                resultado = Some(deslocamento);
            }
        }
        deslocamento += registro.len();
    }
    resultado
}

fn contar_marcadores_ciclo_entre(serial: &[u8], prefixo: &str, inicio: usize, fim: usize) -> usize {
    let mut quantidade = 0usize;
    let mut deslocamento = 0usize;
    for registro in serial.split_inclusive(|b| *b == b'\n') {
        if registro.ends_with(b"\n") {
            let linha = String::from_utf8_lossy(registro);
            let linha = linha.trim_end_matches(['\r', '\n']);
            let linha = linha.strip_prefix("raiz> ").unwrap_or(linha);
            if deslocamento >= inicio && deslocamento < fim && linha.starts_with(prefixo) {
                quantidade += 1;
            }
        }
        deslocamento += registro.len();
    }
    quantidade
}

fn timeouts_esperas_fixture_ciclo(serial: &[u8]) -> Vec<(u64, u64)> {
    let mut resultado = Vec::new();
    for geracao in 1..=4 {
        let referencia = referencia_rede_geracao(geracao);
        let prefixo = format!("[teste-ciclo-espera] DRIVER_ATIVO referencia={referencia:#x}");
        let Some(inicio) = posicao_marcador_ciclo(serial, &prefixo) else {
            continue;
        };
        resultado.extend(
            timeouts_rede_ciclo(&serial[inicio..])
                .into_iter()
                .filter(|(ref_timeout, _)| *ref_timeout == referencia),
        );
    }
    resultado
}

fn validar_snapshot_driver_gen4(serial: &[u8]) -> Resultado<()> {
    let referencia = 0x400000002;
    let antes = snapshot_driver_ciclo(serial, "DRIVER_ATIVO", referencia)
        .ok_or("Snapshot DRIVER_ATIVO gen4 ausente, duplicado ou malformado")?;
    let depois = snapshot_driver_ciclo(serial, "DRIVER_PRESERVADO", referencia)
        .ok_or("Snapshot DRIVER_PRESERVADO gen4 ausente, duplicado ou malformado")?;
    if antes != depois {
        return Err(
            "Estado software/MMIO registrado do driver mudou durante a espera/quota".into(),
        );
    }
    let ativo_prefixo = format!("[teste-ciclo-espera] DRIVER_ATIVO referencia={referencia:#x}");
    let bloqueada_prefixo = format!("[teste-ciclo-espera] BLOQUEADA referencia={referencia:#x}");
    let timeout_prefixo = format!("[teste-ciclo-espera] TIMEOUT referencia={referencia:#x}");
    let recusa_prefixo = "[supervisor] RECURSOS_ESGOTADOS operacao=9 geracao=4";
    let ativo =
        posicao_marcador_ciclo(serial, &ativo_prefixo).ok_or("DRIVER_ATIVO sem posição única")?;
    let recusa = posicao_marcador_ciclo(serial, recusa_prefixo)
        .ok_or("Recusa de quota sem posição única")?;
    let preservado_prefixo =
        format!("[teste-ciclo-espera] DRIVER_PRESERVADO referencia={referencia:#x}");
    let preservado = posicao_marcador_ciclo(serial, &preservado_prefixo)
        .ok_or("DRIVER_PRESERVADO sem posição única")?;
    let bloqueada = posicao_marcador_ciclo_entre(serial, &bloqueada_prefixo, ativo, recusa)
        .ok_or("BLOQUEADA pós-driver e pré-recusa ausente, duplicada ou fora de ordem")?;
    let timeout = posicao_marcador_ciclo_entre(serial, &timeout_prefixo, recusa, preservado)
        .ok_or("TIMEOUT pós-recusa e pré-snapshot ausente, duplicado ou fora de ordem")?;
    if contar_marcadores_ciclo_entre(serial, &timeout_prefixo, ativo, recusa) != 0 {
        return Err("Timeout da espera fixture ocorreu antes da recusa da quota".into());
    }
    if !(ativo < bloqueada && bloqueada < recusa && recusa < timeout && timeout < preservado) {
        return Err(
            "Ordem de driver ativo, espera, recusa, timeout e snapshot preservado inválida".into(),
        );
    }
    Ok(())
}

fn tique_timeout_ciclo(serial: &[u8], referencia: u64) -> Option<u64> {
    let prefixo = format!("[teste-ciclo-espera] TIMEOUT referencia={referencia:#x} tiques=");
    String::from_utf8_lossy(serial)
        .split_inclusive('\n')
        .find_map(|linha| {
            if !linha.ends_with('\n') {
                return None;
            }
            let linha = linha.trim_end_matches(['\r', '\n']);
            let linha = linha.strip_prefix("raiz> ").unwrap_or(linha);
            linha.strip_prefix(&prefixo)?.parse().ok()
        })
}

fn timeouts_rede_ciclo(serial: &[u8]) -> Vec<(u64, u64)> {
    String::from_utf8_lossy(serial)
        .split_inclusive('\n')
        .filter_map(|linha| {
            if !linha.ends_with('\n') {
                return None;
            }
            let linha = linha.trim_end_matches(['\r', '\n']);
            let linha = linha.strip_prefix("raiz> ").unwrap_or(linha);
            let campos = linha.strip_prefix("[teste-ciclo-espera] TIMEOUT ")?;
            let mut campos = campos.split_whitespace();
            let referencia = campos.next()?.strip_prefix("referencia=")?;
            let referencia = u64::from_str_radix(referencia.strip_prefix("0x")?, 16).ok()?;
            let tique = campos.next()?.strip_prefix("tiques=")?.parse().ok()?;
            if campos.next().is_some() || referencia as u32 != 2 || referencia >> 32 == 0 {
                return None;
            }
            Some((referencia, tique))
        })
        .collect()
}

fn janela_timeout_rede_ciclo(
    serial: &[u8],
    inicio: usize,
    apos_recusa: usize,
    referencia_esperada: u64,
    tique_esperado: u64,
) -> Option<Vec<(u64, u64)>> {
    let mut deslocamento = inicio;
    for linha in serial[inicio..].split_inclusive(|b| *b == b'\n') {
        let fim_linha = deslocamento.checked_add(linha.len())?;
        if linha.ends_with(b"\n") {
            let texto = String::from_utf8_lossy(linha);
            let texto = texto.trim_end_matches(['\r', '\n']);
            let texto = texto.strip_prefix("raiz> ").unwrap_or(texto);
            if let Some(campos) = texto.strip_prefix("[teste-ciclo-espera] TIMEOUT ") {
                let mut campos = campos.split_whitespace();
                let referencia = campos.next()?.strip_prefix("referencia=")?;
                let referencia = u64::from_str_radix(referencia.strip_prefix("0x")?, 16).ok()?;
                let tique = campos
                    .next()?
                    .strip_prefix("tiques=")?
                    .parse::<u64>()
                    .ok()?;
                if campos.next().is_some() {
                    return None;
                }
                if referencia == referencia_esperada {
                    if deslocamento < apos_recusa || tique != tique_esperado {
                        return None;
                    }
                    return Some(timeouts_rede_ciclo(&serial[inicio..fim_linha]));
                }
            }
        }
        deslocamento = fim_linha;
    }
    None
}

fn fim_linha(serial: &[u8], esperado: &str) -> Option<usize> {
    let mut fim = 0;
    for linha in serial.split_inclusive(|b| *b == b'\n') {
        fim += linha.len();
        if linha_corresponde(&String::from_utf8_lossy(linha), esperado) {
            return Some(fim);
        }
    }
    None
}

fn testar_ciclo_espera(raiz: &Path) -> Resultado<()> {
    let instante = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_nanos();
    let pasta = raiz.join(format!("evidencias/teste-ciclo-espera-{instante}"));
    fs::create_dir(&pasta)?;
    let resultado = executar_teste_ciclo_espera(
        raiz,
        File::create(pasta.join("serial.log"))?,
        File::create(pasta.join("qemu-stderr.log"))?,
    );
    fs::write(
        pasta.join("resultado.txt"),
        match &resultado {
            Ok(()) => "APROVADO".to_owned(),
            Err(e) => format!("FALHOU: {e}"),
        },
    )?;
    println!("Evidencias: {}", pasta.display());
    resultado
}

fn executar_teste_ciclo_espera(raiz: &Path, serial: File, erros: File) -> Resultado<()> {
    let reserva = UdpSocket::bind("127.0.0.1:17777")?;
    let mut comando = preparar_qemu_em(raiz, false, true, "teste-ciclo-espera")?;
    comando
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    drop(reserva);
    let mut processo = ProcessoQemu(comando.spawn()?);
    let mut entrada = processo.0.stdin.take().ok_or("stdin indisponivel")?;
    let saida = processo.0.stdout.take().ok_or("stdout indisponivel")?;
    let stderr = processo.0.stderr.take().ok_or("stderr indisponivel")?;
    let (emissor, receptor) = mpsc::channel();
    let leitores = [
        leitor_saida(saida, true, emissor.clone()),
        leitor_saida(stderr, false, emissor),
    ];
    let mut captura = Captura {
        eventos: receptor,
        serial: Vec::new(),
        erros: Vec::new(),
        arquivo_serial: serial,
        arquivo_erros: erros,
    };
    let resultado = (|| -> Resultado<()> {
        captura.aguardar(
            &mut processo,
            "[teste] TERMINAL_PRONTO",
            0,
            Duration::from_secs(30),
        )?;
        aguardar_rede_geracao(&mut captura, &mut processo, 0, 1)?;
        captura.aguardar_condicao(
            &mut processo,
            "snapshot do driver ativo da rede 1",
            0,
            Duration::from_secs(30),
            |b| snapshot_driver_ciclo(b, "DRIVER_ATIVO", 0x100000002).is_some(),
        )?;
        validar_pronto_antes_driver_ciclo(&captura.serial, 0x100000002)?;
        captura.aguardar_condicao(
            &mut processo,
            "espera da rede 1",
            0,
            Duration::from_secs(30),
            |b| prazo_ciclo_apos_driver(b, 0x100000002).is_some(),
        )?;
        let prazo_antigo =
            prazo_ciclo_apos_driver(&captura.serial, 0x100000002).ok_or("prazo ausente")?;
        let inicio = enviar_comando(&mut entrada, &mut captura, "parar-rede\n")?;
        captura.aguardar_linha(
            &mut processo,
            "[teste-ciclo-espera] CANCELADA referencia=0x100000002",
            inicio,
            Duration::from_secs(10),
        )?;
        aguardar_evento(
            &mut captura,
            &mut processo,
            inicio,
            "[supervisor] REDE_PARADA",
            &["geracao=1"],
        )?;
        let inicio = enviar_comando(&mut entrada, &mut captura, "reiniciar-rede\n")?;
        aguardar_reinicio_kernel(
            &mut captura,
            &mut processo,
            inicio,
            referencia_rede_geracao(1),
            referencia_rede_geracao(2),
        )?;
        aguardar_rede_geracao(&mut captura, &mut processo, inicio, 2)?;
        captura.aguardar_condicao(
            &mut processo,
            "snapshot do driver ativo da rede 2",
            inicio,
            Duration::from_secs(10),
            |b| snapshot_driver_ciclo(b, "DRIVER_ATIVO", 0x200000002).is_some(),
        )?;
        validar_pronto_antes_driver_ciclo(&captura.serial, 0x200000002)?;
        captura.aguardar_condicao(
            &mut processo,
            "espera de fixture da rede 2 após snapshot",
            inicio,
            Duration::from_secs(10),
            |b| prazo_ciclo_apos_driver(b, 0x200000002).is_some(),
        )?;
        let prazo_novo = prazo_ciclo_apos_driver(&captura.serial[inicio..], 0x200000002)
            .ok_or("prazo novo ausente")?;
        let limite = Instant::now() + Duration::from_secs(30);
        loop {
            let (_, rede, tiques) = consultar_processos(&mut entrada, &mut captura, &mut processo)?;
            if rede != 1 {
                return Err("Rede substituta deixou de estar ativa".into());
            }
            if tiques.wrapping_sub(prazo_antigo) < (1u64 << 63) {
                if tiques.wrapping_sub(prazo_novo) < (1u64 << 63) {
                    return Err("Prazo da rede nova tambem expirou".into());
                }
                break;
            }
            if Instant::now() >= limite {
                return Err("Prazo antigo nao foi alcancado".into());
            }
        }
        conferir(
            timeouts_esperas_fixture_ciclo(&captura.serial).is_empty(),
            "Timeout inesperado de uma instancia da rede antes do prazo legado",
        )?;
        let inicio = enviar_comando(&mut entrada, &mut captura, "teste-ciclo-liberar\n")?;
        captura.aguardar_linha(
            &mut processo,
            "[teste-ciclo] REDE_LIBERADA geracao=2",
            inicio,
            Duration::from_secs(10),
        )?;
        let mut evidencias = EvidenciasSmoke::default();
        conferir_conectividade(
            &mut entrada,
            &mut captura,
            &mut processo,
            2,
            "apos espera antiga",
            &mut evidencias,
        )?;
        let mut inicio_gen4 = None;
        for geracao in 3..=4 {
            let inicio = enviar_comando(&mut entrada, &mut captura, "reiniciar-rede\n")?;
            aguardar_reinicio_kernel(
                &mut captura,
                &mut processo,
                inicio,
                referencia_rede_geracao(geracao - 1),
                referencia_rede_geracao(geracao),
            )?;
            let referencia = referencia_rede_geracao(geracao);
            aguardar_rede_geracao(&mut captura, &mut processo, inicio, geracao)?;
            captura.aguardar_condicao(
                &mut processo,
                "snapshot do driver ativo antes da espera",
                inicio,
                Duration::from_secs(10),
                |b| snapshot_driver_ciclo(b, "DRIVER_ATIVO", referencia).is_some(),
            )?;
            validar_pronto_antes_driver_ciclo(&captura.serial, referencia)?;
            captura.aguardar_condicao(
                &mut processo,
                "espera fixture registrada após inicializar driver",
                inicio,
                Duration::from_secs(10),
                |b| prazo_ciclo_apos_driver(b, referencia).is_some(),
            )?;
            if geracao == 4 {
                inicio_gen4 = posicao_marcador_ciclo(
                    &captura.serial,
                    &format!("[teste-ciclo-espera] DRIVER_ATIVO referencia={referencia:#x}"),
                );
            }
            if geracao == 4 {
                captura.aguardar_linha(
                    &mut processo,
                    "[teste-ciclo-espera] CANCELADA referencia=0x300000002",
                    inicio,
                    Duration::from_secs(10),
                )?;
            }
        }
        let inicio_quota = enviar_comando(&mut entrada, &mut captura, "reiniciar-rede\n")?;
        aguardar_evento(
            &mut captura,
            &mut processo,
            inicio_quota,
            "[supervisor] RECURSOS_ESGOTADOS",
            &["operacao=9", "geracao=4"],
        )?;
        let recusa = "[supervisor] RECURSOS_ESGOTADOS operacao=9 geracao=4";
        let apos_recusa = inicio_quota
            + fim_linha(&captura.serial[inicio_quota..], recusa)
                .ok_or("Linha completa de recusa ausente")?;
        conferir_instancia(&mut entrada, &mut captura, &mut processo, 1, 4)?;
        let prazo_quatro = prazo_ciclo_apos_driver(&captura.serial, 0x400000002)
            .ok_or("prazo da rede 4 ausente após snapshot do driver")?;
        captura.aguardar_condicao(
            &mut processo,
            "timeout da rede 4",
            apos_recusa,
            Duration::from_secs(20),
            |b| tique_timeout_ciclo(b, 0x400000002).is_some(),
        )?;
        let tique_timeout = tique_timeout_ciclo(&captura.serial[apos_recusa..], 0x400000002)
            .ok_or("tique de timeout ausente")?;
        conferir(
            tique_timeout == prazo_quatro,
            "Prazo da rede 4 mudou apos recusa de quota",
        )?;
        conferir(
            janela_timeout_rede_ciclo(
                &captura.serial,
                inicio_gen4.ok_or("Snapshot do driver da geração 4 ausente")?,
                apos_recusa,
                0x400000002,
                prazo_quatro,
            ) == Some(vec![(0x400000002, prazo_quatro)]),
            "Primeiro timeout da geração 4 ocorreu antes da recusa, com identidade/tique incorretos ou após timeout de outra geração",
        )?;
        aguardar_evento(
            &mut captura,
            &mut processo,
            inicio_quota,
            "[teste-ciclo-espera] REDE_TEMPO",
            &["referencia=0x400000002", "geracao=4"],
        )?;
        captura.aguardar_condicao(
            &mut processo,
            "snapshot do mesmo driver após timeout da geração 4",
            inicio_quota,
            Duration::from_secs(10),
            |b| snapshot_driver_ciclo(b, "DRIVER_PRESERVADO", 0x400000002).is_some(),
        )?;
        validar_snapshot_driver_gen4(&captura.serial)?;
        conferir_conectividade(
            &mut entrada,
            &mut captura,
            &mut processo,
            4,
            "apos recusa de quota",
            &mut evidencias,
        )?;
        Ok(())
    })();
    processo.encerrar();
    for leitor in leitores {
        let _ = leitor.join();
    }
    captura.drenar()?;
    resultado?;
    for falha in [
        "FALHA_NUCLEO",
        "SERVICO_FALHOU",
        "SERVICO_FINALIZADO",
        "[teste-ciclo] FALHA",
        "[teste-ciclo-espera] FALHA_DRIVER",
        "panic",
        "PANIC",
    ] {
        if String::from_utf8_lossy(&captura.serial).contains(falha) {
            return Err(format!("Falha observada: {falha}").into());
        }
    }
    if String::from_utf8_lossy(&captura.serial)
        .matches("[supervisor] RECURSOS_ESGOTADOS operacao=9 geracao=4")
        .count()
        != 1
    {
        return Err("Numero inesperado de recusas de quota".into());
    }
    Ok(())
}

fn testar_prazo_idle(raiz: &Path) -> Resultado<()> {
    let instante = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_nanos();
    let pasta = raiz.join(format!("evidencias/teste-prazo-idle-{instante}"));
    fs::create_dir(&pasta)?;
    let resultado = executar_teste_prazo_idle(
        raiz,
        File::create(pasta.join("serial.log"))?,
        File::create(pasta.join("qemu-stderr.log"))?,
        "teste-prazo-idle",
    );
    fs::write(
        pasta.join("resultado.txt"),
        match &resultado {
            Ok(()) => "APROVADO".to_owned(),
            Err(e) => format!("FALHOU: {e}"),
        },
    )?;
    println!("Evidencias: {}", pasta.display());
    resultado
}

fn prazo_idle_ordenado(serial: &[u8]) -> bool {
    let irq = "[teste-prazo-idle] IRQ0_CPL0";
    let timeout = "[teste-prazo] TIMEOUT id=3";
    let resposta = "[teste-prazo-idle] RESPOSTA_CPL3";
    [
        "[teste-ipc] RECEBER_BLOQUEADO id=1",
        "[teste-ipc] RECEBER_BLOQUEADO id=2",
        "[teste-prazo] BLOQUEADO id=3",
    ]
    .iter()
    .all(|bloqueio| linhas_ordenadas(serial, bloqueio, irq))
        && linhas_ordenadas(serial, irq, timeout)
        && linhas_ordenadas(serial, timeout, resposta)
}

fn executar_teste_prazo_idle(
    raiz: &Path,
    serial: File,
    erros: File,
    variante: &str,
) -> Resultado<()> {
    let reserva = UdpSocket::bind("127.0.0.1:17777")?;
    let mut comando = preparar_qemu_em(raiz, false, true, variante)?;
    comando
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    drop(reserva);
    let mut processo = ProcessoQemu(comando.spawn()?);
    let saida = processo.0.stdout.take().ok_or("stdout indisponivel")?;
    let stderr = processo.0.stderr.take().ok_or("stderr indisponivel")?;
    let (emissor, receptor) = mpsc::channel();
    let leitores = [
        leitor_saida(saida, true, emissor.clone()),
        leitor_saida(stderr, false, emissor),
    ];
    let mut captura = Captura {
        eventos: receptor,
        serial: Vec::new(),
        erros: Vec::new(),
        arquivo_serial: serial,
        arquivo_erros: erros,
    };
    let resultado = (|| -> Resultado<()> {
        captura.aguardar_linha(
            &mut processo,
            "[teste-prazo-idle] RESPOSTA_CPL3",
            0,
            Duration::from_secs(30),
        )?;
        conferir(
            prazo_idle_ordenado(&captura.serial),
            "Sequencia de bloqueios, idle, timeout e resposta invalida",
        )?;
        Ok(())
    })();
    processo.encerrar();
    for leitor in leitores {
        let _ = leitor.join();
    }
    captura.drenar()?;
    resultado?;
    for falha in [
        "FALHA_NUCLEO",
        "SERVICO_FALHOU",
        "SERVICO_FINALIZADO",
        "[teste-prazo-idle] FALHA",
        "panic",
        "PANIC",
    ] {
        if String::from_utf8_lossy(&captura.serial).contains(falha) {
            return Err(format!("Falha observada: {falha}").into());
        }
    }
    Ok(())
}

fn testar_prazo(raiz: &Path) -> Resultado<()> {
    let instante = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_nanos();
    let pasta = raiz.join(format!("evidencias/teste-prazo-{instante}"));
    fs::create_dir(&pasta)?;
    let resultado = executar_teste_prazo(
        raiz,
        File::create(pasta.join("serial.log"))?,
        File::create(pasta.join("qemu-stderr.log"))?,
    );
    fs::write(
        pasta.join("resultado.txt"),
        match &resultado {
            Ok(()) => "APROVADO".to_owned(),
            Err(e) => format!("FALHOU: {e}"),
        },
    )?;
    println!("Evidencias: {}", pasta.display());
    resultado
}

fn testar_prazo_empate(raiz: &Path) -> Resultado<()> {
    let instante = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_nanos();
    let pasta = raiz.join(format!("evidencias/teste-prazo-empate-{instante}"));
    fs::create_dir_all(&pasta)?;
    fs::write(
        pasta.join("comando.log"),
        "cargo raiz compilar --teste-prazo-empate\ncargo raiz testar-prazo-empate\ncomando serial: teste-prazo-empate\n",
    )?;
    let resultado = executar_teste_prazo_empate(
        raiz,
        File::create(pasta.join("serial.log"))?,
        File::create(pasta.join("qemu-stderr.log"))?,
    );
    fs::write(
        pasta.join("resultado.txt"),
        match &resultado {
            Ok(()) => "# T03-05 — empate lógico de prazo e IPC\n\nResultado: **APROVADO** para uma espera finita e um envio da rede autorizado, no perfil QEMU/TCG de 1 CPU. O log correlaciona a deadline registrada pelo kernel, a expiração e a admissão real da mensagem no mesmo valor de `TIQUES`, seguida por retorno `TEMPO` e recebimento da IPC em chamada posterior. Isso demonstra somente este interleaving controlado; não demonstra todas as intercalações, justiça para fluxo ilimitado ou comportamento em hardware físico. A mensagem pode ter permanecido na FIFO ou acordado a segunda recepção; o ensaio não distingue essas duas rotas.\n\nEvidências: `serial.log`, `qemu-stderr.log` e `comando.log`.\n".to_owned(),
            Err(e) => format!(
                "# T03-05 — empate lógico de prazo e IPC\n\nResultado: **FALHOU**. `{e}`\n\nO oráculo exige tick efetivo da expiração igual à deadline registrada e tick da admissão, além de retorno único `TEMPO` seguido da mensagem real com origem e payload esperados. Um tick perdido pelo remetente falha fechado. Isso não demonstra justiça para fluxo ilimitado nem comportamento em hardware físico. Evidências parciais: `serial.log`, `qemu-stderr.log` e `comando.log`.\n"
            ),
        },
    )?;
    println!("Evidências: {}", pasta.display());
    resultado
}

fn executar_teste_prazo_empate(raiz: &Path, serial: File, erros: File) -> Resultado<()> {
    conferir(
        raiz.join("saida/teste-prazo-empate/raiz-os.img").is_file(),
        "Imagem ausente; compile primeiro com `cargo raiz compilar --teste-prazo-empate`",
    )?;
    let reserva = UdpSocket::bind("127.0.0.1:17777")
        .map_err(|e| format!("UDP 127.0.0.1:17777 indisponível: {e}"))?;
    let mut comando = preparar_qemu_em(raiz, false, true, "teste-prazo-empate")?;
    comando
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    drop(reserva);
    let mut processo = ProcessoQemu(comando.spawn()?);
    let mut entrada = processo.0.stdin.take().ok_or("stdin QEMU indisponível")?;
    let saida = processo.0.stdout.take().ok_or("stdout QEMU indisponível")?;
    let stderr = processo.0.stderr.take().ok_or("stderr QEMU indisponível")?;
    let (emissor, receptor) = mpsc::channel();
    let leitores = [
        leitor_saida(saida, true, emissor.clone()),
        leitor_saida(stderr, false, emissor),
    ];
    let mut captura = Captura {
        eventos: receptor,
        serial: Vec::new(),
        erros: Vec::new(),
        arquivo_serial: serial,
        arquivo_erros: erros,
    };
    let resultado = (|| -> Resultado<()> {
        captura.aguardar_linha(
            &mut processo,
            "[teste-prazo-empate] KERNEL_FEATURE=1",
            0,
            Duration::from_secs(30),
        )?;
        captura.aguardar(
            &mut processo,
            "[teste] TERMINAL_PRONTO",
            0,
            Duration::from_secs(30),
        )?;
        aguardar_rede_geracao(&mut captura, &mut processo, 0, 1)?;
        captura.aguardar(
            &mut processo,
            "[supervisor] SERVICO_PRONTO",
            0,
            Duration::from_secs(30),
        )?;
        captura.drenar()?;
        let inicio = enviar_comando(&mut entrada, &mut captura, "teste-prazo-empate\n")?;
        captura.aguardar_condicao(
            &mut processo,
            "resultado ou falha T03-05 completo",
            inicio,
            Duration::from_secs(30),
            |bytes| {
                linha_com_prefixo_completa(bytes, "[teste-prazo-empate] RESULTADO")
                    || linha_com_prefixo_completa(bytes, "[teste-prazo-empate] FALHA")
                    || linha_com_prefixo_completa(bytes, "[microkernel] FALHA_NUCLEO")
                    || linha_com_prefixo_completa(bytes, "[microkernel] SERVICO_FALHOU")
            },
        )?;
        captura.drenar()?;
        validar_log_prazo_empate(&captura.serial)
    })();
    drop(entrada);
    processo.encerrar();
    let mut erro_finalizacao = None;
    for leitor in leitores {
        if leitor.join().is_err() {
            erro_finalizacao = Some("Falha na thread de captura do QEMU".to_owned());
        }
    }
    if let Err(erro) = captura.drenar() {
        erro_finalizacao = Some(erro.to_string());
    }
    captura.arquivo_serial.flush()?;
    captura.arquivo_erros.flush()?;
    match resultado {
        Err(erro) => Err(erro),
        Ok(()) => match erro_finalizacao {
            Some(erro) => Err(erro.into()),
            None => Ok(()),
        },
    }
}

fn executar_teste_prazo(raiz: &Path, serial: File, erros: File) -> Resultado<()> {
    let reserva = UdpSocket::bind("127.0.0.1:17777")?;
    let mut comando = preparar_qemu_em(raiz, false, true, "teste-prazo")?;
    comando
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    drop(reserva);
    let mut processo = ProcessoQemu(comando.spawn()?);
    let mut entrada = processo.0.stdin.take().ok_or("stdin indisponivel")?;
    let saida = processo.0.stdout.take().ok_or("stdout indisponivel")?;
    let stderr = processo.0.stderr.take().ok_or("stderr indisponivel")?;
    let (emissor, receptor) = mpsc::channel();
    let leitores = [
        leitor_saida(saida, true, emissor.clone()),
        leitor_saida(stderr, false, emissor),
    ];
    let mut captura = Captura {
        eventos: receptor,
        serial: Vec::new(),
        erros: Vec::new(),
        arquivo_serial: serial,
        arquivo_erros: erros,
    };
    let resultado = (|| -> Resultado<()> {
        captura.aguardar_linha(
            &mut processo,
            "[teste-prazo-servico] ETAPA 1 CPL3",
            0,
            Duration::from_secs(30),
        )?;
        captura.aguardar_linha(
            &mut processo,
            "[teste-prazo] BLOQUEADO id=3",
            0,
            Duration::from_secs(30),
        )?;
        let inicio = enviar_comando(&mut entrada, &mut captura, "teste-prazo-enviar 1\n")?;
        captura.aguardar_linha(
            &mut processo,
            "[teste-prazo-servico] ETAPA 2 CPL3",
            inicio,
            Duration::from_secs(10),
        )?;
        captura.aguardar_linha(
            &mut processo,
            "[teste-prazo-servico] ETAPA 3 CPL3",
            inicio,
            Duration::from_secs(10),
        )?;
        conferir(
            linhas_ordenadas(
                &captura.serial[inicio..],
                "[teste-prazo] MENSAGEM id=3",
                "[teste-prazo] TIMEOUT id=3",
            ),
            "Mensagem e timeout nao observados na ordem",
        )?;
        conferir(
            linhas_ordenadas(
                &captura.serial[inicio..],
                "[teste-prazo-servico] ETAPA 2 CPL3",
                "[teste-prazo-servico] ETAPA 3 CPL3",
            ),
            "Etapas de servico fora de ordem",
        )?;
        let tardia = enviar_comando(&mut entrada, &mut captura, "teste-prazo-enviar 2\n")?;
        captura.aguardar_linha(
            &mut processo,
            "[teste-prazo-servico] ETAPA 4 CPL3",
            tardia,
            Duration::from_secs(10),
        )?;
        Ok(())
    })();
    processo.encerrar();
    for leitor in leitores {
        let _ = leitor.join();
    }
    captura.drenar()?;
    resultado?;
    for falha in [
        "FALHA_NUCLEO",
        "SERVICO_FALHOU",
        "[teste-prazo-servico] FALHA",
        "panic",
        "PANIC",
    ] {
        if String::from_utf8_lossy(&captura.serial).contains(falha) {
            return Err(format!("Falha observada: {falha}").into());
        }
    }
    Ok(())
}

fn testar_receber_bloqueante(raiz: &Path) -> Resultado<()> {
    let instante = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_nanos();
    let pasta = raiz.join(format!("evidencias/teste-receber-bloqueante-{instante}"));
    fs::create_dir(&pasta)?;
    let resultado = executar_teste_receber_bloqueante(
        raiz,
        File::create(pasta.join("serial.log"))?,
        File::create(pasta.join("qemu-stderr.log"))?,
    );
    fs::write(
        pasta.join("resultado.txt"),
        match &resultado {
            Ok(()) => "APROVADO".to_owned(),
            Err(e) => format!("FALHOU: {e}"),
        },
    )?;
    println!("Evidencias: {}", pasta.display());
    resultado
}

fn executar_teste_receber_bloqueante(raiz: &Path, serial: File, erros: File) -> Resultado<()> {
    let reserva = UdpSocket::bind("127.0.0.1:17777")?;
    let mut comando = preparar_qemu_em(raiz, false, true, "teste-receber-bloqueante")?;
    comando
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    drop(reserva);
    let mut processo = ProcessoQemu(comando.spawn()?);
    let mut entrada = processo.0.stdin.take().ok_or("stdin indisponivel")?;
    let saida = processo.0.stdout.take().ok_or("stdout indisponivel")?;
    let stderr = processo.0.stderr.take().ok_or("stderr indisponivel")?;
    let (emissor, receptor) = mpsc::channel();
    let leitores = [
        leitor_saida(saida, true, emissor.clone()),
        leitor_saida(stderr, false, emissor),
    ];
    let mut captura = Captura {
        eventos: receptor,
        serial: Vec::new(),
        erros: Vec::new(),
        arquivo_serial: serial,
        arquivo_erros: erros,
    };
    let resultado = (|| -> Resultado<()> {
        captura.aguardar(
            &mut processo,
            "[teste] TERMINAL_PRONTO",
            0,
            Duration::from_secs(30),
        )?;
        captura.aguardar_linha(
            &mut processo,
            "[teste-ipc] BLOQUEADO id=3",
            0,
            Duration::from_secs(30),
        )?;
        let inicio = enviar_comando(&mut entrada, &mut captura, "teste-receber-preparar\n")?;
        captura.aguardar_linha(
            &mut processo,
            "[teste-ipc] RECEBER_IMEDIATO id=3",
            inicio,
            Duration::from_secs(10),
        )?;
        captura.aguardar_linha(
            &mut processo,
            "[teste-ipc] RECEBER_BLOQUEADO id=3",
            inicio,
            Duration::from_secs(10),
        )?;
        conferir(
            linhas_ordenadas(
                &captura.serial[inicio..],
                "[teste-ipc] RECEBER_IMEDIATO id=3",
                "[teste-ipc] RECEBER_BLOQUEADO id=3",
            ),
            "Recebimento imediato e bloqueio fora de ordem",
        )?;
        thread::sleep(Duration::from_millis(500));
        captura.drenar()?;
        conferir(
            !String::from_utf8_lossy(&captura.serial[inicio..])
                .contains("[teste-ipc] RECEBER_CONCLUIDO id=3"),
            "Recebimento concluiu antes do envio",
        )?;
        let inicio = enviar_comando(&mut entrada, &mut captura, "teste-receber-enviar\n")?;
        captura.aguardar_linha(
            &mut processo,
            "[teste-ipc] RECEBER_CONCLUIDO id=3",
            inicio,
            Duration::from_secs(10),
        )?;
        captura.aguardar_linha(
            &mut processo,
            "[teste-receber] RESPOSTA_FIFO_CPL3",
            inicio,
            Duration::from_secs(10),
        )?;
        conferir(
            linhas_ordenadas(
                &captura.serial[inicio..],
                "[teste-ipc] RECEBER_CONCLUIDO id=3",
                "[teste-receber] RESPOSTA_FIFO_CPL3",
            ),
            "Resposta anterior a conclusao do recebimento",
        )?;
        Ok(())
    })();
    processo.encerrar();
    for leitor in leitores {
        let _ = leitor.join();
    }
    captura.drenar()?;
    resultado?;
    for falha in [
        "FALHA_NUCLEO",
        "SERVICO_FALHOU",
        "[teste-receber] FALHA",
        "panic",
        "PANIC",
    ] {
        if String::from_utf8_lossy(&captura.serial).contains(falha) {
            return Err(format!("Falha observada: {falha}").into());
        }
    }
    Ok(())
}

fn marcadores_ipc_ordenados(serial: &[u8]) -> bool {
    linhas_ordenadas(
        serial,
        "[teste-ipc] DESPERTADO id=3",
        "[teste-ipc] RESPOSTA_FIFO_CPL3",
    )
}

fn linha_corresponde(linha: &str, esperado: &str) -> bool {
    if !linha.ends_with('\n') {
        return false;
    }
    let linha = linha.trim_end_matches(['\r', '\n']);
    linha == esperado
        || ((esperado.starts_with("[teste-ipc] ")
            || esperado.starts_with("[teste-prazo] ")
            || esperado.starts_with("[teste-ciclo-espera] ")
            || esperado.starts_with("[teste-irq] ")
            || esperado.starts_with("[teste-uart] ")
            || esperado.starts_with("[teste-uart-rz025] ")
            || esperado.starts_with("[teste-vida] "))
            && linha.strip_prefix("raiz> ") == Some(esperado))
}

fn linhas_ordenadas(serial: &[u8], primeira: &str, segunda: &str) -> bool {
    let texto = String::from_utf8_lossy(serial);
    let mut despertou = false;
    for linha in texto.split_inclusive('\n') {
        if linha_corresponde(linha, primeira) {
            despertou = true;
        } else if linha_corresponde(linha, segunda) && despertou {
            return true;
        }
    }
    false
}

fn executar_teste_ipc_bloqueante(raiz: &Path, serial: File, erros: File) -> Resultado<()> {
    let reserva = UdpSocket::bind("127.0.0.1:17777")?;
    let mut comando = preparar_qemu_em(raiz, false, true, "teste-ipc-bloqueante")?;
    comando
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    drop(reserva);
    let mut processo = ProcessoQemu(comando.spawn()?);
    let mut entrada = processo.0.stdin.take().ok_or("stdin indisponivel")?;
    let saida = processo.0.stdout.take().ok_or("stdout indisponivel")?;
    let stderr = processo.0.stderr.take().ok_or("stderr indisponivel")?;
    let (emissor, receptor) = mpsc::channel();
    let leitores = [
        leitor_saida(saida, true, emissor.clone()),
        leitor_saida(stderr, false, emissor),
    ];
    let mut captura = Captura {
        eventos: receptor,
        serial: Vec::new(),
        erros: Vec::new(),
        arquivo_serial: serial,
        arquivo_erros: erros,
    };
    let resultado = (|| -> Resultado<()> {
        captura.aguardar(
            &mut processo,
            "[teste] TERMINAL_PRONTO",
            0,
            Duration::from_secs(30),
        )?;
        captura.aguardar_linha(
            &mut processo,
            "[teste-ipc] BLOQUEADO id=3",
            0,
            Duration::from_secs(30),
        )?;
        thread::sleep(Duration::from_millis(500));
        captura.drenar()?;
        conferir(
            !String::from_utf8_lossy(&captura.serial).contains("[teste-ipc] DESPERTADO id=3"),
            "Despertar prematuro",
        )?;
        let inicio = enviar_comando(&mut entrada, &mut captura, "teste-ipc-bloqueante\n")?;
        captura.aguardar(
            &mut processo,
            "[teste-ipc] DESPERTADO id=3",
            inicio,
            Duration::from_secs(10),
        )?;
        captura.aguardar_linha(
            &mut processo,
            "[teste-ipc] RESPOSTA_FIFO_CPL3",
            inicio,
            Duration::from_secs(10),
        )?;
        conferir(
            marcadores_ipc_ordenados(&captura.serial[inicio..]),
            "Resposta IPC anterior ao despertar ou marcadores incompletos",
        )?;
        // O supervisor pode bloquear novamente antes de o terminal imprimir a
        // resposta; procure a partir do último despertar da etapa anterior.
        captura.drenar()?;
        let despertar = b"[teste-ipc] DESPERTADO id=3";
        let apos_despertar = captura
            .serial
            .windows(despertar.len())
            .rposition(|w| w == despertar)
            .ok_or("Despertar anterior ausente")?
            + despertar.len();
        captura.aguardar_linha(
            &mut processo,
            "[teste-ipc] RECEBER_BLOQUEADO id=3",
            apos_despertar,
            Duration::from_secs(10),
        )?;
        let inicio = enviar_comando(&mut entrada, &mut captura, "supervisao\n")?;
        captura.aguardar_linha(
            &mut processo,
            "[supervisor] MODO manual reinicios=0 ultimo_erro=0x0 limite=4",
            inicio,
            Duration::from_secs(10),
        )?;
        conferir(
            linhas_ordenadas(
                &captura.serial[inicio..],
                "[teste-ipc] RECEBER_CONCLUIDO id=3",
                "[supervisor] MODO manual reinicios=0 ultimo_erro=0x0 limite=4",
            ),
            "Consulta de supervisao sem despertar anterior",
        )?;
        Ok(())
    })();
    processo.encerrar();
    for leitor in leitores {
        let _ = leitor.join();
    }
    captura.drenar()?;
    resultado?;
    for falha in [
        "FALHA_NUCLEO",
        "SERVICO_FALHOU",
        "[teste-ipc] FALHA",
        "panic",
        "PANIC",
    ] {
        if String::from_utf8_lossy(&captura.serial).contains(falha) {
            return Err(format!("Falha observada: {falha}").into());
        }
    }
    Ok(())
}

const MARCADORES_IDLE: [&str; 4] = [
    "[teste-idle] TODOS_BLOQUEADOS",
    "[teste-idle] IRQ0_CPL0",
    "[teste-idle] SERVICOS_PRONTOS",
    "[teste-idle] RETORNO_CPL3",
];

fn testar_idle(raiz: &Path) -> Resultado<()> {
    let inicio = Instant::now();
    let pasta = raiz.join("evidencias");
    fs::create_dir_all(&pasta)?;
    let serial = File::create(pasta.join("serial-teste-idle.log"))?;
    let erros = File::create(pasta.join("qemu-teste-idle-stderr.log"))?;
    let mut etapas = Vec::new();
    let resultado = executar_teste_idle(raiz, serial, erros, &mut etapas);
    let mut relatorio = format!(
        "# Teste controlado de idle\n\nResultado: **{}**. Duração: {:.3} segundos.\n\nAmbiente solicitado: QEMU/TCG, pc, 1 CPU, 256 MiB, OVMF e e1000. Imagem: `saida/teste-idle/raiz-os.img`.\n\n## Marcadores observados\n\nOs quatro marcadores de teste são exigidos na ordem indicada. TERMINAL_PRONTO deve ocorrer após SERVICOS_PRONTOS, podendo ocorrer antes ou depois de RETORNO_CPL3.\n\n",
        if resultado.is_ok() {
            "APROVADO"
        } else {
            "FALHOU"
        },
        inicio.elapsed().as_secs_f64()
    );
    for etapa in &etapas {
        relatorio.push_str(&format!("- `{etapa}`\n"));
    }
    if let Err(erro) = &resultado {
        relatorio.push_str(&format!("\n## Falha observada\n\n```text\n{erro}\n```\n"));
    }
    relatorio.push_str("\nLogs: `serial-teste-idle.log` e `qemu-teste-idle-stderr.log`. O cenário instrumentado é opt-in e não demonstra espera por eventos dos serviços nem desempenho geral.\n");
    fs::write(pasta.join("RELATORIO-TESTE-IDLE.md"), relatorio)?;
    resultado
}

fn executar_teste_idle(
    raiz: &Path,
    serial: File,
    erros: File,
    etapas: &mut Vec<String>,
) -> Resultado<()> {
    let reserva = UdpSocket::bind("127.0.0.1:17777")
        .map_err(|e| format!("UDP 127.0.0.1:17777 indisponível: {e}"))?;
    let mut comando = preparar_qemu_em(raiz, false, true, "teste-idle")?;
    comando
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    drop(reserva);
    let mut processo = ProcessoQemu(comando.spawn()?);
    let saida = processo
        .0
        .stdout
        .take()
        .ok_or("stdout do emulador indisponível")?;
    let stderr = processo
        .0
        .stderr
        .take()
        .ok_or("stderr do emulador indisponível")?;
    let (emissor, receptor) = mpsc::channel();
    let leitores = [
        leitor_saida(saida, true, emissor.clone()),
        leitor_saida(stderr, false, emissor),
    ];
    let mut captura = Captura {
        eventos: receptor,
        serial: Vec::new(),
        erros: Vec::new(),
        arquivo_serial: serial,
        arquivo_erros: erros,
    };
    let resultado = (|| -> Resultado<()> {
        let mut desde = 0;
        let mut desde_servicos = 0;
        for marcador in MARCADORES_IDLE {
            captura.aguardar_linha(&mut processo, marcador, desde, Duration::from_secs(30))?;
            let trecho = &captura.serial[desde..];
            let posicao = trecho
                .windows(marcador.len())
                .position(|w| w == marcador.as_bytes())
                .ok_or("Marcador desapareceu da captura")?;
            desde += posicao + marcador.len();
            if marcador == "[teste-idle] SERVICOS_PRONTOS" {
                desde_servicos = desde;
            }
            etapas.push(marcador.to_owned());
        }
        captura.aguardar(
            &mut processo,
            "[teste] TERMINAL_PRONTO",
            desde_servicos,
            Duration::from_secs(30),
        )?;
        etapas.push("[teste] TERMINAL_PRONTO".into());
        Ok(())
    })();
    processo.encerrar();
    for leitor in leitores {
        let _ = leitor.join();
    }
    captura.drenar()?;
    resultado?;
    for falha in ["FALHA_NUCLEO", "SERVICO_FALHOU"] {
        if captura
            .serial
            .windows(falha.len())
            .any(|w| w == falha.as_bytes())
        {
            return Err(format!("Marcador de falha observado: {falha}").into());
        }
    }
    Ok(())
}

#[derive(Default)]
struct EvidenciasSmoke {
    etapas: Vec<String>,
    pacotes_udp: usize,
    bytes_udp: usize,
}

fn testar(raiz: &Path) -> Resultado<()> {
    let inicio = Instant::now();
    let pasta = raiz.join("evidencias");
    fs::create_dir_all(&pasta)?;
    let arquivo_serial = File::create(pasta.join("serial-smoke.log"))?;
    let arquivo_erros = File::create(pasta.join("qemu-smoke-stderr.log"))?;
    let mut evidencias = EvidenciasSmoke::default();
    println!("Testando boot, serviços em CPL=3, rede e isolamento de falha em QEMU/TCG...");
    let resultado = executar_smoke(raiz, arquivo_serial, arquivo_erros, &mut evidencias);
    let mut relatorio = format!(
        "# Teste de integração do Raiz OS\n\nResultado: **{}**.\n\nDuração: {:.3} segundos.\n\nAmbiente solicitado: QEMU/TCG, pc, 1 CPU, 256 MiB, OVMF, e1000.\n\n## Etapas concluídas\n\n",
        if resultado.is_ok() {
            "APROVADO"
        } else {
            "FALHOU"
        },
        inicio.elapsed().as_secs_f64()
    );
    if evidencias.etapas.is_empty() {
        relatorio.push_str("Nenhuma etapa concluída.\n");
    } else {
        for etapa in &evidencias.etapas {
            relatorio.push_str(&format!("- {etapa}\n"));
        }
    }
    relatorio.push_str(&format!("\nDatagramas UDP com resposta idêntica verificada: **{}**. Bytes de payload verificados: **{}**.\n",
        evidencias.pacotes_udp, evidencias.bytes_udp));
    if let Err(erro) = &resultado {
        relatorio.push_str(&format!("\n## Falha observada\n\n```text\n{erro}\n```\n"));
    }
    relatorio.push_str("\n## Evidências e alcance\n\n- `serial-smoke.log`: bytes recebidos do console serial.\n- `qemu-smoke-stderr.log`: mensagens do emulador.\n\nO comando encerra o emulador também em caso de falha. A instrução inválida do serviço de rede é provocada intencionalmente. O caso de isolamento só passa quando o terminal continua respondendo, o estado do serviço indica falha e um novo envio IPC à rede recebe o erro esperado.\n\nEste resultado cobre a plataforma virtual declarada e os casos executados. O isolamento observado é parcial: não comprova IOMMU, contenção de DMA, reinício automático de serviços, preempção sob qualquer carga, suporte a hardware físico, TCP, persistência ou prontidão para produção.\n");
    fs::write(pasta.join("RELATORIO-SMOKE.md"), relatorio)?;
    println!("Relatório: {}", pasta.join("RELATORIO-SMOKE.md").display());
    resultado
}

fn executar_smoke(
    raiz: &Path,
    arquivo_serial: File,
    arquivo_erros: File,
    evidencias: &mut EvidenciasSmoke,
) -> Resultado<()> {
    // Uma instância interativa utiliza a mesma porta. Não reutilizar um guest já
    // aberto: o teste precisa observar seu próprio boot e termina sua própria VM.
    let reserva_porta = UdpSocket::bind("127.0.0.1:17777").map_err(|e| {
        format!("UDP 127.0.0.1:17777 indisponível; encerre a VM interativa antes do teste: {e}")
    })?;
    let mut comando = preparar_qemu(raiz, false, true)?;
    evidencias
        .etapas
        .push("Imagem verificada: GPT, FAT32 e BOOTX64.EFI íntegros.".into());
    comando
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    drop(reserva_porta);
    let mut processo = ProcessoQemu(comando.spawn()?);
    let mut entrada = processo
        .0
        .stdin
        .take()
        .ok_or("stdin do emulador indisponível")?;
    let saida = processo
        .0
        .stdout
        .take()
        .ok_or("stdout do emulador indisponível")?;
    let erros = processo
        .0
        .stderr
        .take()
        .ok_or("stderr do emulador indisponível")?;
    let (emissor, receptor) = mpsc::channel();
    let leitores = [
        leitor_saida(saida, true, emissor.clone()),
        leitor_saida(erros, false, emissor),
    ];
    let mut captura = Captura {
        eventos: receptor,
        serial: Vec::new(),
        erros: Vec::new(),
        arquivo_serial,
        arquivo_erros,
    };
    let resultado = (|| -> Resultado<()> {
        captura.aguardar(
            &mut processo,
            "[teste] TERMINAL_PRONTO",
            0,
            Duration::from_secs(30),
        )?;
        captura.aguardar(
            &mut processo,
            "[rede] SERVICO_PRONTO CPL=3",
            0,
            Duration::from_secs(10),
        )?;
        conferir(
            captura
                .serial
                .windows(b"EXIT_BOOT_SERVICES_OK".len())
                .any(|w| w == b"EXIT_BOOT_SERVICES_OK"),
            "O marcador de ExitBootServices não foi observado",
        )?;
        evidencias.etapas.push("Boot completado: ExitBootServices, TERMINAL_PRONTO e serviço de rede em CPL=3 observados.".into());

        captura.drenar()?;
        let inicio_sobre = captura.serial.len();
        enviar_serial(&mut entrada, b"sobre\n")?;
        captura.aguardar(&mut processo, "CPL=3", inicio_sobre, Duration::from_secs(5))?;
        captura.aguardar(
            &mut processo,
            "raiz> ",
            inicio_sobre,
            Duration::from_secs(5),
        )?;
        let inicio_memoria = enviar_comando(&mut entrada, &mut captura, "memoria\n")?;
        captura.aguardar(
            &mut processo,
            "Tres servicos usam o mesmo VA",
            inicio_memoria,
            Duration::from_secs(5),
        )?;
        captura.aguardar(
            &mut processo,
            "raiz> ",
            inicio_memoria,
            Duration::from_secs(5),
        )?;
        let inicio_ping = enviar_comando(&mut entrada, &mut captura, "ping 10.0.2.2\n")?;
        captura.aguardar(
            &mut processo,
            "PING OK 10.0.2.2",
            inicio_ping,
            Duration::from_secs(10),
        )?;
        evidencias.etapas.push(
            "Comando sobre informou CPL=3 do terminal; ICMP echo do gateway 10.0.2.2 recebido."
                .into(),
        );

        let socket = socket_eco()?;
        for tamanho in [0usize, 1, 17, 1472] {
            let dados: Vec<u8> = (0..tamanho)
                .map(|i| ((i * 31 + tamanho) % 251) as u8)
                .collect();
            conferir_eco(&socket, &dados)?;
            evidencias.pacotes_udp += 1;
            evidencias.bytes_udp += dados.len();
        }
        evidencias
            .etapas
            .push("UDP eco verificado com payloads de 0, 1, 17 e 1472 bytes.".into());
        for numero in 0..80usize {
            let dados: Vec<u8> = (0..64usize)
                .map(|i| ((numero * 17 + i * 31) % 251) as u8)
                .collect();
            conferir_eco(&socket, &dados)?;
            evidencias.pacotes_udp += 1;
            evidencias.bytes_udp += dados.len();
        }
        evidencias.etapas.push("80 datagramas UDP adicionais verificados sequencialmente, cobrindo a volta dos anéis RX/TX.".into());
        captura.drenar()?;
        let inicio_contadores = captura.serial.len();
        enviar_serial(&mut entrada, b"rede\n")?;
        captura.aguardar(
            &mut processo,
            &format!("udp_eco={}", evidencias.pacotes_udp),
            inicio_contadores,
            Duration::from_secs(5),
        )?;
        let contadores = String::from_utf8_lossy(&captura.serial[inicio_contadores..]);
        conferir(
            contadores.contains("RX=")
                && contadores.contains("TX=")
                && contadores.contains("ping_ok=1"),
            "Contadores de rede esperados não foram observados",
        )?;
        evidencias.etapas.push(format!(
            "Comando rede confirmou RX/TX, ping_ok=1 e udp_eco={}.",
            evidencias.pacotes_udp
        ));

        let inicio_capacidade = captura.serial.len();
        enviar_serial(&mut entrada, b"teste-cap\n")?;
        captura.aguardar(
            &mut processo,
            "[teste] CAPACIDADE_NEGADA_OK",
            inicio_capacidade,
            Duration::from_secs(5),
        )?;
        evidencias
            .etapas
            .push("Acesso sem capacidade foi negado e confirmado pelo comando teste-cap.".into());

        let inicio_falha = captura.serial.len();
        enviar_serial(&mut entrada, b"falhar-rede\n")?;
        captura.aguardar(
            &mut processo,
            "[microkernel] SERVICO_FALHOU id=2 vetor=6",
            inicio_falha,
            Duration::from_secs(5),
        )?;
        evidencias.etapas.push(
            "Instrução inválida provocada no serviço de rede gerou falha id=2, vetor=6.".into(),
        );

        let inicio_sobrevivencia = captura.serial.len();
        enviar_serial(
            &mut entrada,
            b"eco TERMINAL_VIVO_APOS_FALHA\nprocessos\nping 10.0.2.2\n",
        )?;
        captura.aguardar_linha(
            &mut processo,
            "TERMINAL_VIVO_APOS_FALHA",
            inicio_sobrevivencia,
            Duration::from_secs(5),
        )?;
        captura.aguardar(
            &mut processo,
            "rede=2",
            inicio_sobrevivencia,
            Duration::from_secs(5),
        )?;
        captura.aguardar(
            &mut processo,
            "[teste] IPC_SERVICO_INDISPONIVEL_OK",
            inicio_sobrevivencia,
            Duration::from_secs(5),
        )?;
        evidencias.etapas.push("Após a morte da rede, terminal respondeu ao eco, processos informou rede=2 e o novo ping recebeu erro de serviço indisponível por IPC.".into());
        Ok(())
    })();
    drop(entrada);
    processo.encerrar();
    let mut erro_finalizacao = None;
    for leitor in leitores {
        if leitor.join().is_err() {
            erro_finalizacao = Some("Falha na thread de captura do emulador".into());
        }
    }
    if let Err(erro) = captura.drenar() {
        erro_finalizacao = Some(erro);
    }
    captura.arquivo_serial.flush()?;
    captura.arquivo_erros.flush()?;
    match resultado {
        Err(erro) => Err(erro),
        Ok(()) => match erro_finalizacao {
            Some(erro) => Err(erro),
            None => Ok(()),
        },
    }
}

#[derive(Clone, Copy)]
enum CenarioIsolamento {
    Memoria,
    Preempcao,
}

fn testar_isolamento(raiz: &Path) -> Resultado<()> {
    let inicio = Instant::now();
    let pasta = raiz.join("evidencias");
    fs::create_dir_all(&pasta)?;
    let mut memoria = Vec::new();
    let mut preempcao = Vec::new();
    println!("Testando proteção de memória em uma nova VM...");
    let resultado_memoria =
        executar_caso_isolamento(raiz, CenarioIsolamento::Memoria, &mut memoria);
    println!("Testando preempção em outra VM...");
    let resultado_preempcao =
        executar_caso_isolamento(raiz, CenarioIsolamento::Preempcao, &mut preempcao);
    let aprovado = resultado_memoria.is_ok() && resultado_preempcao.is_ok();
    let mut relatorio = format!(
        "# Testes de memória e preempção do Raiz OS\n\nResultado geral: **{}**.\n\nDuração: {:.3} segundos.\n\nAmbiente solicitado: duas inicializações independentes e sequenciais em QEMU/TCG, pc, 1 CPU, 256 MiB, OVMF e e1000.\n\n",
        if aprovado { "APROVADO" } else { "FALHOU" },
        inicio.elapsed().as_secs_f64()
    );
    for (titulo, etapas, resultado, prefixo) in [
        (
            "Proteção de memória",
            &memoria,
            &resultado_memoria,
            "memoria",
        ),
        ("Preempção", &preempcao, &resultado_preempcao, "preempcao"),
    ] {
        relatorio.push_str(&format!(
            "## {titulo}\n\nResultado: **{}**.\n\n",
            if resultado.is_ok() {
                "APROVADO"
            } else {
                "FALHOU"
            }
        ));
        if etapas.is_empty() {
            relatorio.push_str("Nenhuma etapa concluída.\n");
        }
        for etapa in etapas {
            relatorio.push_str(&format!("- {etapa}\n"));
        }
        if let Err(erro) = resultado {
            relatorio.push_str(&format!("\nFalha observada:\n\n```text\n{erro}\n```\n"));
        }
        relatorio.push_str(&format!("\nLogs: `serial-isolamento-{prefixo}.log` e `qemu-isolamento-{prefixo}-stderr.log`.\n\n"));
    }
    relatorio.push_str("## Alcance da evidência\n\nO caso de memória tenta uma leitura de CPU do endereço 0x02000000 pelo serviço de rede. Exige page fault do serviço 2 (vetor 14), CR2=0x2000000 e continuidade do terminal. Ele não testa acesso por DMA nem configura IOMMU.\n\nO caso de preempção coloca a rede em loop sem chamadas voluntárias de cessão. Exige resposta do terminal, os dois serviços ativos e aumento dos tiques entre duas consultas separadas por pelo menos 200 ms no host. Isso evidencia a preempção nesse cenário; não é uma medição de latência máxima ou garantia de tempo real.\n\nOs emuladores são encerrados após cada caso, inclusive em erro. Estes arquivos não substituem os logs e o relatório do comando `testar`. Os resultados não afirmam reinício automático, cobertura de todos os acessos de memória, contenção de dispositivos ou prontidão para produção.\n");
    let destino = pasta.join("RELATORIO-ISOLAMENTO.md");
    fs::write(&destino, relatorio)?;
    println!("Relatório: {}", destino.display());
    if aprovado {
        Ok(())
    } else {
        let mut falhas = Vec::new();
        if let Err(erro) = resultado_memoria {
            falhas.push(format!("memória: {erro}"));
        }
        if let Err(erro) = resultado_preempcao {
            falhas.push(format!("preempção: {erro}"));
        }
        Err(format!("Teste de isolamento falhou: {}", falhas.join("; ")).into())
    }
}

fn executar_caso_isolamento(
    raiz: &Path,
    cenario: CenarioIsolamento,
    etapas: &mut Vec<String>,
) -> Resultado<()> {
    let prefixo = match cenario {
        CenarioIsolamento::Memoria => "memoria",
        CenarioIsolamento::Preempcao => "preempcao",
    };
    let pasta = raiz.join("evidencias");
    let arquivo_serial = File::create(pasta.join(format!("serial-isolamento-{prefixo}.log")))?;
    let arquivo_erros = File::create(pasta.join(format!("qemu-isolamento-{prefixo}-stderr.log")))?;
    let reserva_porta = UdpSocket::bind("127.0.0.1:17777").map_err(|e| {
        format!("UDP 127.0.0.1:17777 indisponível; encerre a VM interativa antes do teste: {e}")
    })?;
    let mut comando = preparar_qemu(raiz, false, true)?;
    etapas.push("Imagem GPT/FAT32 e bytes de BOOTX64.EFI verificados antes do boot.".into());
    comando
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    drop(reserva_porta);
    let mut processo = ProcessoQemu(comando.spawn()?);
    let mut entrada = processo
        .0
        .stdin
        .take()
        .ok_or("stdin do emulador indisponível")?;
    let saida = processo
        .0
        .stdout
        .take()
        .ok_or("stdout do emulador indisponível")?;
    let erros = processo
        .0
        .stderr
        .take()
        .ok_or("stderr do emulador indisponível")?;
    let (emissor, receptor) = mpsc::channel();
    let leitores = [
        leitor_saida(saida, true, emissor.clone()),
        leitor_saida(erros, false, emissor),
    ];
    let mut captura = Captura {
        eventos: receptor,
        serial: Vec::new(),
        erros: Vec::new(),
        arquivo_serial,
        arquivo_erros,
    };
    let resultado = (|| -> Resultado<()> {
        captura.aguardar(
            &mut processo,
            "[teste] TERMINAL_PRONTO",
            0,
            Duration::from_secs(30),
        )?;
        captura.aguardar(
            &mut processo,
            "[rede] SERVICO_PRONTO CPL=3",
            0,
            Duration::from_secs(10),
        )?;
        conferir(
            captura
                .serial
                .windows(b"EXIT_BOOT_SERVICES_OK".len())
                .any(|w| w == b"EXIT_BOOT_SERVICES_OK"),
            "O marcador de ExitBootServices não foi observado",
        )?;
        etapas.push(
            "Nova inicialização: ExitBootServices, terminal e serviço de rede em CPL=3 observados."
                .into(),
        );
        captura.drenar()?;
        match cenario {
            CenarioIsolamento::Memoria => {
                let inicio_falha = captura.serial.len();
                enviar_serial(&mut entrada, b"violar-memoria\n")?;
                captura.aguardar_condicao(
                    &mut processo,
                    "page fault id=2 vetor=14 CR2=0x2000000",
                    inicio_falha,
                    Duration::from_secs(10),
                    |bytes| {
                        String::from_utf8_lossy(bytes)
                            .split_inclusive('\n')
                            .any(|linha| {
                                linha.ends_with('\n')
                                    && linha.contains("[microkernel] SERVICO_FALHOU id=2 vetor=14")
                                    && linha
                                        .split_whitespace()
                                        .any(|campo| campo == "CR2=0x2000000")
                            })
                    },
                )?;
                etapas.push("Leitura de 0x02000000 pelo serviço de rede gerou page fault id=2, vetor=14, CR2=0x2000000.".into());
                let inicio_eco = captura.serial.len();
                enviar_serial(&mut entrada, b"eco MEMORIA_PROTEGIDA_TERMINAL_VIVO\n")?;
                captura.aguardar_linha(
                    &mut processo,
                    "MEMORIA_PROTEGIDA_TERMINAL_VIVO",
                    inicio_eco,
                    Duration::from_secs(10),
                )?;
                let estado = consultar_processos(&mut entrada, &mut captura, &mut processo)?;
                if estado.0 != 1 || estado.1 != 2 {
                    return Err(format!("Após page fault, esperado terminal=1 rede=2; observado terminal={} rede={} tiques={}", estado.0, estado.1, estado.2).into());
                }
                etapas.push(format!("Terminal respondeu ao eco após o page fault; estados terminal={} rede={}, tiques={}.", estado.0, estado.1, estado.2));
            }
            CenarioIsolamento::Preempcao => {
                let inicio_loop = captura.serial.len();
                enviar_serial(&mut entrada, b"ocupar-rede\n")?;
                captura.aguardar(
                    &mut processo,
                    "[rede] LOOP_SEM_CEDER",
                    inicio_loop,
                    Duration::from_secs(10),
                )?;
                etapas.push(
                    "Serviço de rede confirmou entrada no loop sem cessão voluntária.".into(),
                );
                thread::sleep(Duration::from_millis(200));
                captura.drenar()?;
                let inicio_eco = captura.serial.len();
                enviar_serial(&mut entrada, b"eco PREEMPCAO_OK\n")?;
                captura.aguardar_linha(
                    &mut processo,
                    "PREEMPCAO_OK",
                    inicio_eco,
                    Duration::from_secs(10),
                )?;
                etapas.push("Terminal respondeu com linha exata PREEMPCAO_OK enquanto a rede permanecia no loop.".into());
                let primeiro = consultar_processos(&mut entrada, &mut captura, &mut processo)?;
                if primeiro.0 != 1 || primeiro.1 != 1 {
                    return Err(format!("Primeira consulta: esperado terminal=1 rede=1; observado terminal={} rede={} tiques={}", primeiro.0, primeiro.1, primeiro.2).into());
                }
                etapas.push(format!(
                    "Primeira consulta: terminal=1 rede=1 tiques={}.",
                    primeiro.2
                ));
                thread::sleep(Duration::from_millis(200));
                let segundo = consultar_processos(&mut entrada, &mut captura, &mut processo)?;
                if segundo.0 != 1 || segundo.1 != 1 || segundo.2 <= primeiro.2 {
                    return Err(format!("Segunda consulta: esperado serviços ativos e tiques maiores que {}; observado terminal={} rede={} tiques={}", primeiro.2, segundo.0, segundo.1, segundo.2).into());
                }
                etapas.push(format!("Segunda consulta após 200 ms no host: terminal=1 rede=1 tiques={}; aumento de {} tiques.", segundo.2, segundo.2 - primeiro.2));
            }
        }
        Ok(())
    })();
    drop(entrada);
    processo.encerrar();
    let mut erro_finalizacao = None;
    for leitor in leitores {
        if leitor.join().is_err() {
            erro_finalizacao = Some("Falha na thread de captura do emulador".into());
        }
    }
    if let Err(erro) = captura.drenar() {
        erro_finalizacao = Some(erro);
    }
    captura.arquivo_serial.flush()?;
    captura.arquivo_erros.flush()?;
    match resultado {
        Err(erro) => Err(erro),
        Ok(()) => match erro_finalizacao {
            Some(erro) => Err(erro),
            None => Ok(()),
        },
    }
}

fn consultar_processos(
    entrada: &mut std::process::ChildStdin,
    captura: &mut Captura,
    processo: &mut ProcessoQemu,
) -> Resultado<(u64, u64, u64)> {
    captura.drenar()?;
    let inicio = captura.serial.len();
    enviar_serial(entrada, b"processos\n")?;
    captura.aguardar_condicao(
        processo,
        "resposta completa de processos",
        inicio,
        Duration::from_secs(10),
        |bytes| ultimo_estado_processos(bytes).is_some(),
    )?;
    ultimo_estado_processos(&captura.serial[inicio..])
        .ok_or_else(|| "Resposta de processos ausente após marcador".into())
}

fn ultimo_estado_processos(bytes: &[u8]) -> Option<(u64, u64, u64)> {
    String::from_utf8_lossy(bytes)
        .split_inclusive('\n')
        .filter(|linha| linha.ends_with('\n'))
        .filter_map(|linha| {
            let mut campos = linha.split_whitespace();
            let terminal = campos.next()?.strip_prefix("terminal=")?.parse().ok()?;
            let rede = campos.next()?.strip_prefix("rede=")?.parse().ok()?;
            let tiques = campos.next()?.strip_prefix("tiques=")?.parse().ok()?;
            if campos.next().is_some() {
                return None;
            }
            Some((terminal, rede, tiques))
        })
        .last()
}

#[derive(Clone, Copy)]
enum CenarioRecuperacao {
    CicloCompleto,
    RedeEmLoop,
}

fn testar_recuperacao(raiz: &Path) -> Resultado<()> {
    let inicio = Instant::now();
    let pasta = raiz.join("evidencias");
    fs::create_dir_all(&pasta)?;
    let mut ciclo = EvidenciasSmoke::default();
    let mut ciclo_loop = EvidenciasSmoke::default();
    println!("Testando supervisor, gerações de referências e recuperação da rede...");
    let resultado_ciclo =
        executar_caso_recuperacao(raiz, CenarioRecuperacao::CicloCompleto, &mut ciclo);
    println!("Testando recuperação de um serviço preso em loop, em outra VM...");
    let resultado_loop =
        executar_caso_recuperacao(raiz, CenarioRecuperacao::RedeEmLoop, &mut ciclo_loop);
    let aprovado = resultado_ciclo.is_ok() && resultado_loop.is_ok();
    let mut relatorio = format!(
        "# Recuperação de serviços — Raiz OS 0.2\n\nResultado geral: **{}**.\n\nDuração: {:.3} segundos.\n\nAmbiente solicitado: QEMU/TCG, pc, 1 CPU, 256 MiB, OVMF e e1000. Dois boots independentes e sequenciais.\n\n",
        if aprovado { "APROVADO" } else { "FALHOU" },
        inicio.elapsed().as_secs_f64()
    );
    for (titulo, evidencias, resultado, prefixo) in [
        (
            "Supervisor, referências e limite de recursos",
            &ciclo,
            &resultado_ciclo,
            "recuperacao",
        ),
        (
            "Recuperação de rede em loop sem cessão voluntária",
            &ciclo_loop,
            &resultado_loop,
            "recuperacao-loop",
        ),
    ] {
        relatorio.push_str(&format!(
            "## {titulo}\n\nResultado: **{}**.\n\n",
            if resultado.is_ok() {
                "APROVADO"
            } else {
                "FALHOU"
            }
        ));
        if evidencias.etapas.is_empty() {
            relatorio.push_str("Nenhuma etapa concluída.\n");
        }
        for etapa in &evidencias.etapas {
            relatorio.push_str(&format!("- {etapa}\n"));
        }
        relatorio.push_str(&format!("\nRespostas UDP idênticas ao envio verificadas: {}. Bytes de payload conferidos: {}.\n", evidencias.pacotes_udp, evidencias.bytes_udp));
        if let Err(erro) = resultado {
            relatorio.push_str(&format!("\nFalha observada:\n\n```text\n{erro}\n```\n"));
        }
        relatorio.push_str(&format!(
            "\nLogs: `serial-{prefixo}.log` e `qemu-{prefixo}-stderr.log`.\n\n"
        ));
    }
    relatorio.push_str("## Alcance e limites\n\nO ciclo principal exerce recuperação manual após instrução inválida, recuperação automática após page fault, recusa de referências de uma geração antiga, autoridade exclusiva do supervisor para reiniciar/encerrar, parada voluntária e retomada. Na quarta encarnação, solicita uma quinta e exige recusa por recursos sem perda da instância ativa. A rede precisa responder a ping e UDP após cada recuperação e também após a recusa.\n\nO segundo boot testa o reinício de uma rede presa em loop sem cessão voluntária. Ele possui orçamento próprio de encarnações e não amplia artificialmente o orçamento do primeiro cenário.\n\nEsses testes não comprovam contenção por IOMMU, proteção contra DMA malicioso, política de reinícios ilimitados, restauração de conexões TCP, persistência de estado de aplicações ou prontidão para produção. As arenas antigas não são recicladas no cenário testado; o limite de quatro encarnações é parte do comportamento observado.\n\nOs emuladores são encerrados após cada caso, inclusive em erro. Os relatórios e logs dos comandos `testar` e `testar-isolamento` são preservados.\n");
    let destino = pasta.join("RELATORIO-RECUPERACAO.md");
    fs::write(&destino, relatorio)?;
    println!("Relatório: {}", destino.display());
    if aprovado {
        Ok(())
    } else {
        let mut erros = Vec::new();
        if let Err(erro) = resultado_ciclo {
            erros.push(format!("ciclo principal: {erro}"));
        }
        if let Err(erro) = resultado_loop {
            erros.push(format!("recuperação de loop: {erro}"));
        }
        Err(format!("Teste de recuperação falhou: {}", erros.join("; ")).into())
    }
}

fn testar_irq_ipc(raiz: &Path) -> Resultado<()> {
    let instante = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_nanos();
    let pasta = raiz.join(format!("evidencias/teste-irq-ipc-{instante}"));
    fs::create_dir(&pasta)?;
    let resultado = executar_teste_irq_ipc(
        raiz,
        File::create(pasta.join("serial.log"))?,
        File::create(pasta.join("qemu-stderr.log"))?,
        "teste-irq-ipc",
    );
    fs::write(
        pasta.join("resultado.txt"),
        match &resultado {
            Ok(()) => "APROVADO: oito IPCs em ordem; trabalho serial e IPC observados na janela finita após IRQ4; eco completo.".to_owned(),
            Err(e) => format!("FALHOU: {e}"),
        },
    )?;
    println!("Evidências: {}", pasta.display());
    resultado
}

/// Executa três boots de escopo finito: despertar real por IRQ4, prazo real e
/// pressão serial/IPC já coberta pela fixture de oito mensagens. Cada imagem
/// fica em diretório próprio; os artefatos e hashes são guardados por cenário.
fn testar_rz024(raiz: &Path) -> Resultado<()> {
    let instante = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_nanos();
    let pasta = raiz.join(format!("evidencias/teste-rz024-{instante}"));
    fs::create_dir_all(&pasta)?;
    let resultado = executar_teste_rz024(raiz, &pasta);
    fs::write(
        pasta.join("resultado.txt"),
        match &resultado {
            Ok(()) => "APROVADO: boots controlados IRQ4, prazo e pressão finita; ver manifests e logs por cenário. Sem meta de desempenho ou inferência de energia/hardware físico.".to_owned(),
            Err(e) => format!("FALHOU: {e}"),
        },
    )?;
    println!("Evidências: {}", pasta.display());
    resultado
}

fn executar_teste_rz024(raiz: &Path, pasta: &Path) -> Resultado<()> {
    for (cenario, variante) in [
        ("irq4", "teste-rz024-irq"),
        ("prazo", "teste-rz024-prazo"),
        ("pressao-finita", "teste-rz024-pressao"),
    ] {
        let destino = pasta.join(cenario);
        fs::create_dir_all(&destino)?;
        compilar(raiz, variante)?;
        registrar_ambiente_rz024(raiz, variante, &destino)?;
        let serial_path = destino.join("serial.log");
        let erros_path = destino.join("qemu-stderr.log");
        let resultado = match cenario {
            "irq4" => executar_teste_irq(
                raiz,
                File::create(&serial_path)?,
                File::create(&erros_path)?,
                variante,
            ),
            "prazo" => executar_teste_prazo_idle(
                raiz,
                File::create(&serial_path)?,
                File::create(&erros_path)?,
                variante,
            ),
            _ => executar_teste_irq_ipc(
                raiz,
                File::create(&serial_path)?,
                File::create(&erros_path)?,
                variante,
            ),
        };
        let serial = fs::read(&serial_path)?;
        let verificacao = match cenario {
            "irq4" => validar_idle_rz024(&serial, "IRQ4"),
            "prazo" => validar_idle_rz024(&serial, "IRQ0"),
            _ => Ok(()),
        };
        let resultado_cenario = resultado.and(verificacao);
        fs::write(
            destino.join("resultado.txt"),
            match &resultado_cenario {
                Ok(()) if cenario == "pressao-finita" => "APROVADO: fixture existente confirmou oito IPCs ordenadas e eco completo; carga finita, sem meta de desempenho.".to_owned(),
                Ok(()) => format!("APROVADO: cenário {cenario}; observação delimitada no serial.log."),
                Err(e) => format!("FALHOU: {e}"),
            },
        )?;
        resultado_cenario?;
    }
    Ok(())
}

fn registrar_ambiente_rz024(raiz: &Path, variante: &str, pasta: &Path) -> Resultado<()> {
    let imagem = raiz.join(format!("saida/{variante}/raiz-os.img"));
    let firmware = localizar_firmware()?;
    let qemu_nome = env::var_os("RAIZ_QEMU").unwrap_or_else(|| "qemu-system-x86_64".into());
    let qemu = Command::new(&qemu_nome).arg("--version").output()?;
    if !qemu.status.success() {
        return Err("Não foi possível obter a versão do QEMU configurado".into());
    }
    let qemu_versao = String::from_utf8_lossy(&qemu.stdout);
    let variaveis = localizar_variaveis(&firmware);
    let mut manifesto = format!(
        "cenario={variante}\nsha256_imagem={}\nqemu_versao={}\nmaquina=pc accel=tcg smp=1 memoria=256M rede=e1000 serial=stdio monitor=none display=none no-reboot snapshot=on hostfwd=udp:17777\nsha256_ovmf={}\n",
        sha256_arquivo(&imagem)?,
        qemu_versao.lines().next().unwrap_or("versao-desconhecida"),
        sha256_arquivo(&firmware)?,
    );
    if let Some(caminho) = variaveis {
        manifesto.push_str(&format!("sha256_ovmf_vars={}\n", sha256_arquivo(&caminho)?));
    } else {
        manifesto.push_str("ovmf_vars=nao-localizado\nsha256_ovmf_vars=indisponivel\n");
    }
    manifesto.push_str(&format!(
        "qemu_dados_configurado={}\n",
        if env::var_os("RAIZ_QEMU_DADOS").is_some() {
            "sim"
        } else {
            "nao"
        }
    ));
    fs::write(pasta.join("ambiente.txt"), manifesto)?;
    Ok(())
}

fn sha256_arquivo(caminho: &Path) -> Resultado<String> {
    #[cfg(windows)]
    let saida = Command::new("certutil")
        .args(["-hashfile"])
        .arg(caminho)
        .arg("SHA256")
        .output()?;
    #[cfg(not(windows))]
    let saida = Command::new("sha256sum").arg(caminho).output()?;
    if !saida.status.success() {
        return Err(format!("Falha ao calcular SHA-256 de {}", caminho.display()).into());
    }
    let texto = String::from_utf8_lossy(&saida.stdout);
    #[cfg(windows)]
    let formato = "certutil";
    #[cfg(not(windows))]
    let formato = "sha256sum";
    extrair_hash_sha256(&texto, formato)
        .map(|hash| hash.to_ascii_lowercase())
        .map_err(|_| format!("SHA-256 não reconhecido para {}", caminho.display()).into())
}

fn extrair_hash_sha256<'a>(texto: &'a str, formato: &str) -> Resultado<&'a str> {
    for linha in texto.lines().map(str::trim) {
        match formato {
            "certutil"
                if linha.len() == 64 && linha.bytes().all(|byte| byte.is_ascii_hexdigit()) =>
            {
                return Ok(linha);
            }
            "sha256sum" if linha.len() > 66 => {
                let (Some(hash), Some(separador)) = (linha.get(0..64), linha.get(64..66)) else {
                    continue;
                };
                if hash.bytes().all(|byte| byte.is_ascii_hexdigit())
                    && (separador == "  " || separador == " *")
                {
                    return Ok(hash);
                }
            }
            _ => {}
        }
    }
    Err("Formato SHA-256 inválido".into())
}

fn validar_idle_rz024(serial: &[u8], fonte_esperada: &str) -> Resultado<()> {
    let texto = std::str::from_utf8(serial)?;
    if fragmento_final_prefixo_marcador_rz024(texto) {
        return Err("Fragmento final é prefixo de marcador reservado do ensaio".into());
    }
    let inicios: Vec<_> = texto
        .lines()
        .filter(|linha| linha.starts_with("[teste-rz024] IDLE_INICIO "))
        .collect();
    let fins: Vec<_> = texto
        .lines()
        .filter(|linha| linha.starts_with("[teste-rz024] IDLE_FIM "))
        .collect();
    let armadas: Vec<_> = texto
        .lines()
        .filter(|linha| linha.starts_with("[teste-rz024] JANELA_ARMADA "))
        .collect();
    if inicios.len() != 1 || fins.len() != 1 || armadas.len() != 1 {
        return Err("Esperava exatamente um marcador completo de início e fim de idle".into());
    }
    if armadas
        .iter()
        .chain(inicios.iter())
        .chain(fins.iter())
        .any(|linha| !linha_serial_completa(serial, linha))
    {
        return Err("Marcador RZ-024 incompleto".into());
    }
    let pos_armada = texto.find(armadas[0]).ok_or("Armação ausente")?;
    let pos_inicio = texto.find(inicios[0]).ok_or("Início idle ausente")?;
    let pos_fim = texto.find(fins[0]).ok_or("Fim idle ausente")?;
    if pos_armada >= pos_inicio || pos_inicio >= pos_fim {
        return Err("Armação e resumo idle estão fora de ordem".into());
    }
    if fonte_esperada == "IRQ4"
        && (!texto.lines().any(|linha| linha == "[teste-irq] BLOQUEADO")
            || texto
                .lines()
                .filter(|linha| linha.starts_with("[teste-irq] IRQ_ENTREGUE"))
                .count()
                != 1)
    {
        return Err("Fixture IRQ não mostrou bloqueio e entrega ao terminal".into());
    }
    if fonte_esperada == "IRQ0" && !prazo_idle_ordenado(serial) {
        return Err(
            "Fixture de prazo não mostrou bloqueios, IRQ0, timeout e resposta em ordem".into(),
        );
    }
    let cenario_esperado = if fonte_esperada == "IRQ4" {
        "irq4"
    } else {
        "prazo"
    };
    let armada = campos_exatos_rz024(armadas[0], "JANELA_ARMADA", &["cenario", "referencia"])?;
    let inicio = campos_exatos_rz024(
        inicios[0],
        "IDLE_INICIO",
        &[
            "cenario",
            "janela",
            "entradas_idle",
            "executaveis",
            "terminal",
            "rede",
            "supervisor",
        ],
    )?;
    let fim = campos_exatos_rz024(
        fins[0],
        "IDLE_FIM",
        &[
            "cenario",
            "janela",
            "fonte",
            "entradas_idle",
            "tiques_irq0_idle",
            "entradas_irq4_idle",
            "mascara_irq4",
            "selecionado",
        ],
    )?;
    if armada["cenario"] != cenario_esperado
        || inicio["cenario"] != cenario_esperado
        || fim["cenario"] != cenario_esperado
    {
        return Err("Marcadores pertencem a outro cenário".into());
    }
    if inicio["janela"] != "1" || inicio["entradas_idle"] != "1" || inicio["executaveis"] != "0" {
        return Err("A entrada idle não prova scheduler sem executáveis na primeira janela".into());
    }
    let refs = ["terminal", "rede", "supervisor"].map(|chave| {
        u64::from_str_radix(inicio[chave].strip_prefix("0x").unwrap_or(""), 16).unwrap_or(0)
    });
    if refs
        .iter()
        .enumerate()
        .any(|(indice, ref_)| *ref_ == 0 || *ref_ >> 32 == 0 || *ref_ as u32 != indice as u32 + 1)
    {
        return Err("Referências não identificam as gerações de terminal/rede/supervisor".into());
    }
    let referencia_armada =
        u64::from_str_radix(armada["referencia"].strip_prefix("0x").unwrap_or(""), 16).unwrap_or(0);
    if referencia_armada != refs[0] {
        return Err("Armação não pertence à instância atual do terminal".into());
    }
    if fim["janela"] != "1" || fim["entradas_idle"] != "1" || fim["fonte"] != fonte_esperada {
        return Err("A saída de idle não corresponde à fonte esperada".into());
    }
    if fonte_esperada == "IRQ4" {
        let linha_irq = texto
            .lines()
            .find(|linha| linha.starts_with("[teste-irq] IRQ_ENTREGUE"))
            .ok_or("Entrega IRQ4 ausente")?;
        if !linha_serial_completa(serial, "[teste-irq] BLOQUEADO")
            || !linha_serial_completa(serial, linha_irq)
        {
            return Err("Marcador de bloqueio ou entrega IRQ incompleto".into());
        }
        let sequencia = linha_irq
            .strip_prefix("[teste-irq] IRQ_ENTREGUE seq=")
            .ok_or("Marcador IRQ4 malformado")?;
        if sequencia.is_empty() || !sequencia.bytes().all(|byte| byte.is_ascii_digit()) {
            return Err("Sequência IRQ4 não é decimal exata".into());
        }
        if sequencia.parse::<u64>()? == 0
            || texto.find(fins[0]).ok_or("Resumo idle ausente")?
                >= texto.find(linha_irq).ok_or("Entrega IRQ ausente")?
        {
            return Err("Sequência IRQ4 inválida ou não correlacionada à saída de idle".into());
        }
    }
    let tique = decimal_exato_rz024(fim["tiques_irq0_idle"])?;
    let irq4 = decimal_exato_rz024(fim["entradas_irq4_idle"])?;
    let mascara = decimal_exato_rz024(fim["mascara_irq4"])?;
    let selecionado =
        u64::from_str_radix(fim["selecionado"].strip_prefix("0x").unwrap_or(""), 16).unwrap_or(0);
    match fonte_esperada {
        "IRQ4" if irq4 != 1 || selecionado != refs[0] || mascara != 1 => {
            return Err("Despertar IRQ4 não correlaciona byte/terminal e máscara observados".into())
        }
        "IRQ0" if tique == 0 || irq4 != 0 || selecionado != refs[2] => {
            return Err("Despertar por prazo não correlaciona tique e supervisor".into())
        }
        _ => {}
    }
    Ok(())
}

fn linha_serial_completa(serial: &[u8], esperada: &str) -> bool {
    serial.split_inclusive(|byte| *byte == b'\n').any(|linha| {
        if !linha.ends_with(b"\n") {
            return false;
        }
        let linha = linha.strip_suffix(b"\n").unwrap_or(linha);
        let linha = linha.strip_suffix(b"\r").unwrap_or(linha);
        linha == esperada.as_bytes()
    })
}

fn fragmento_final_prefixo_marcador_rz024(serial: &str) -> bool {
    let fragmento = serial
        .rsplit('\n')
        .next()
        .unwrap_or("")
        .trim_end_matches('\r');
    if fragmento.is_empty() {
        return false;
    }
    [
        "[teste-rz024] JANELA_ARMADA",
        "[teste-rz024] IDLE_INICIO",
        "[teste-rz024] IDLE_FIM",
        "[teste-irq] BLOQUEADO",
        "[teste-irq] IRQ_ENTREGUE",
        "[teste-prazo] BLOQUEADO",
        "[teste-prazo] TIMEOUT",
        "[teste-prazo-idle] IRQ0_CPL0",
        "[teste-prazo-idle] RESPOSTA_CPL3",
    ]
    .iter()
    .any(|marcador| marcador.starts_with(fragmento))
}

fn campos_exatos_rz024<'a>(
    linha: &'a str,
    marcador: &str,
    esperados: &[&str],
) -> Resultado<std::collections::BTreeMap<&'a str, &'a str>> {
    let prefixo = format!("[teste-rz024] {marcador} ");
    let corpo = linha
        .strip_prefix(&prefixo)
        .ok_or("Prefixo RZ-024 inválido")?;
    let mut campos = std::collections::BTreeMap::new();
    for token in corpo.split_ascii_whitespace() {
        let (chave, valor) = token.split_once('=').ok_or("Campo RZ-024 malformado")?;
        if !esperados.contains(&chave) || valor.is_empty() || campos.insert(chave, valor).is_some()
        {
            return Err("Campo RZ-024 inesperado, vazio ou duplicado".into());
        }
    }
    if campos.len() != esperados.len() {
        return Err("Campos RZ-024 ausentes".into());
    }
    Ok(campos)
}

fn decimal_exato_rz024(valor: &str) -> Resultado<u32> {
    if valor.is_empty() || !valor.bytes().all(|b| b.is_ascii_digit()) {
        return Err("Contador RZ-024 não é decimal exato".into());
    }
    Ok(valor.parse()?)
}

fn testar_irq_morte(raiz: &Path) -> Resultado<()> {
    let instante = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_nanos();
    let pasta = raiz.join(format!("evidencias/teste-irq-morte-{instante}"));
    fs::create_dir(&pasta)?;
    let resultado = executar_teste_irq_morte(
        raiz,
        File::create(pasta.join("serial.log"))?,
        File::create(pasta.join("qemu-stderr.log"))?,
    );
    fs::write(
        pasta.join("resultado.txt"),
        match &resultado {
            Ok(()) => "APROVADO: terminal recebeu IRQ4 real e falhou sem confirmar; kernel registrou a revogação da associação e a máscara, 50 amostras limitadas de IRQ0 e aceitou o sinal opt-in do supervisor após o serviço receber FALHOU. A rede respondeu a eco UDP antes e depois da falha. A amostragem é delimitada, não contínua; o resultado não identifica qual serviço executou entre as amostras, não testa reinicialização do terminal e não prova ausência de IRQ4 fora da janela observada.".to_owned(),
            Err(e) => format!("FALHOU: {e}"),
        },
    )?;
    println!("Evidências: {}", pasta.display());
    resultado
}

fn executar_teste_irq_morte(raiz: &Path, serial: File, erros: File) -> Resultado<()> {
    let reserva = UdpSocket::bind("127.0.0.1:17777")?;
    let mut comando = preparar_qemu_em(raiz, false, true, "teste-irq-morte")?;
    comando
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    drop(reserva);
    let mut processo = ProcessoQemu(comando.spawn()?);
    let mut entrada = processo.0.stdin.take().ok_or("stdin indisponível")?;
    let saida = processo.0.stdout.take().ok_or("stdout indisponível")?;
    let stderr = processo.0.stderr.take().ok_or("stderr indisponível")?;
    let (emissor, receptor) = mpsc::channel();
    let leitores = [
        leitor_saida(saida, true, emissor.clone()),
        leitor_saida(stderr, false, emissor),
    ];
    let mut captura = Captura {
        eventos: receptor,
        serial: Vec::new(),
        erros: Vec::new(),
        arquivo_serial: serial,
        arquivo_erros: erros,
    };
    let resultado = (|| -> Resultado<()> {
        captura.aguardar(
            &mut processo,
            "[teste] TERMINAL_PRONTO",
            0,
            Duration::from_secs(30),
        )?;
        aguardar_rede_geracao(&mut captura, &mut processo, 0, 1)?;
        captura.aguardar(
            &mut processo,
            "[supervisor] SERVICO_PRONTO",
            0,
            Duration::from_secs(30),
        )?;
        let inicio_ping = enviar_comando(&mut entrada, &mut captura, "ping 10.0.2.2\n")?;
        captura.aguardar(
            &mut processo,
            "PING OK 10.0.2.2",
            inicio_ping,
            Duration::from_secs(15),
        )?;
        let socket = socket_eco()?;
        conferir_eco(&socket, b"irq-morte-preflight")?;
        let inicio = enviar_comando(&mut entrada, &mut captura, "teste-irq-morte\n")?;
        captura.aguardar_linha(
            &mut processo,
            "[teste-irq-morte] ARMADA CPL3",
            inicio,
            Duration::from_secs(10),
        )?;
        enviar_serial(&mut entrada, b"A")?;
        captura.aguardar(
            &mut processo,
            "[microkernel] SERVICO_FALHOU id=1 vetor=6",
            inicio,
            Duration::from_secs(15),
        )?;
        aguardar_evento(
            &mut captura,
            &mut processo,
            inicio,
            "[teste-irq-morte] TIMER_AMOSTRAS",
            &[
                "quantidade=50",
                "mascara_ok=50",
                "associacao_ausente_ok=50",
                "entradas_irq4=0",
            ],
        )?;
        aguardar_evento(
            &mut captura,
            &mut processo,
            inicio,
            "[teste-irq-morte] SUPERVISOR_REAGIU_FALHOU",
            &["referencia=0x100000001"],
        )?;
        conferir_eco(&socket, b"irq-morte-rede-apos-falha")?;
        captura.drenar()?;
        validar_log_irq_morte(&String::from_utf8_lossy(&captura.serial))?;
        Ok(())
    })();
    processo.encerrar();
    for leitor in leitores {
        let _ = leitor.join();
    }
    resultado?;
    Ok(())
}

fn validar_log_irq_morte(log: &str) -> Resultado<()> {
    let linhas: Vec<&str> = log.split_inclusive('\n').collect();
    let pos = |marcador: &str| -> Resultado<usize> {
        let encontrados: Vec<usize> = linhas
            .iter()
            .enumerate()
            .filter_map(|(indice, linha)| {
                (linha.trim_start().starts_with(marcador) && linha.ends_with('\n'))
                    .then_some(indice)
            })
            .collect();
        if encontrados.len() != 1 {
            return Err(format!(
                "marcador ausente, incompleto ou duplicado ({marcador}): {}",
                encontrados.len()
            )
            .into());
        }
        Ok(encontrados[0])
    };
    let armada = pos("[teste-irq-morte] ARMADA CPL3")?;
    let recebido = pos("[teste-irq-morte] IRQ4_RECEBIDA seq=")?;
    let revogacao_inicio = pos("[teste-irq-morte] IRQ4_REVOGACAO_INICIO ")?;
    let revogada = pos("[teste-irq-morte] IRQ4_REVOGADA ")?;
    let falha = pos("[microkernel] SERVICO_FALHOU id=1 vetor=6")?;
    let supervisor = pos("[teste-irq-morte] SUPERVISOR_REAGIU_FALHOU ")?;
    let amostras = pos("[teste-irq-morte] TIMER_AMOSTRAS ")?;
    conferir(
        armada < recebido
            && recebido < revogacao_inicio
            && revogacao_inicio < revogada
            && revogada < falha
            && falha < supervisor
            && falha < amostras,
        "Ordem inesperada entre entrega, revogação, falha e observações",
    )?;
    let linha_recebida = linhas[recebido];
    let seq = campo_decimal(linha_recebida, "seq=")?;
    conferir(
        seq != 0 && campo_hex(linha_recebida, "byte=")? == 0x41,
        "Sequência IRQ4 ou byte serial inválido",
    )?;
    let referencia = campo_hex(linhas[revogacao_inicio], "referencia=")?;
    conferir(
        campo_decimal(linhas[revogacao_inicio], "seq=")? == seq
            && campo_decimal(linhas[revogacao_inicio], "mascara=")? == 1,
        "Revogação não corresponde à sequência entregue com IRQ4 já mascarada",
    )?;
    conferir(
        campo_hex(linhas[falha], "referencia=")? == referencia
            && campo_hex(linhas[supervisor], "referencia=")? == referencia,
        "Falha/sinal do supervisor não correspondem à instância revogada",
    )?;
    conferir(
        campo_decimal(linhas[falha], "id=")? == 1
            && campo_decimal(linhas[falha], "vetor=")? == 6
            && log.matches("[microkernel] SERVICO_FALHOU").count() == 1,
        "A falha observada não é a única falha do terminal por UD2",
    )?;
    conferir(
        campo_decimal(linhas[revogada], "referencia=")? == 0
            && campo_decimal(linhas[revogada], "mascara=")? == 1,
        "Associação IRQ4 não foi observada vazia com IRQ4 mascarada",
    )?;
    let linha_amostras = linhas[amostras];
    conferir(
        campo_decimal(linha_amostras, "quantidade=")? == 50
            && campo_decimal(linha_amostras, "mascara_ok=")? == 50
            && campo_decimal(linha_amostras, "associacao_ausente_ok=")? == 50,
        "A amostragem limitada não confirmou máscara e associação nas 50 amostras",
    )?;
    let desde_falha = campo_decimal(linha_amostras, "desde_falha=")?;
    conferir(
        desde_falha >= 50,
        "Timer não avançou 50 tiques após a falha",
    )?;
    let primeiro_tique = campo_decimal(linha_amostras, "tique_inicial=")?;
    let ultimo_tique = campo_decimal(linha_amostras, "tique_final=")?;
    conferir(
        ultimo_tique.wrapping_sub(primeiro_tique) == 49,
        "As 50 amostras não cobriram 50 tiques IRQ0 consecutivos",
    )?;
    conferir(
        campo_decimal(linha_amostras, "entradas_irq4=")? == 0,
        "Foi observada entrada IRQ4 durante as 50 amostras",
    )?;
    conferir(
        !log.contains("[microkernel] SERVICO_REINICIADO")
            && !log.contains("[microkernel] SERVICO_FINALIZADO id=1")
            && log.matches("[teste] TERMINAL_PRONTO").count() == 1,
        "O log contém reinício/reentrada inesperados do terminal",
    )
}

fn campo_decimal(linha: &str, nome: &str) -> Resultado<u64> {
    let valor = campo_unico(linha, nome)?;
    valor
        .parse::<u64>()
        .map_err(|_| format!("campo decimal inválido: {nome}{valor}").into())
}

fn campo_hex(linha: &str, nome: &str) -> Resultado<u64> {
    let valor = campo_unico(linha, nome)?;
    let valor = valor.strip_prefix("0x").unwrap_or(valor);
    u64::from_str_radix(valor, 16)
        .map_err(|_| format!("campo hexadecimal inválido: {nome}{valor}").into())
}

fn campo_unico<'a>(linha: &'a str, nome: &str) -> Resultado<&'a str> {
    let valores: Vec<&str> = linha
        .split_whitespace()
        .filter_map(|campo| campo.strip_prefix(nome))
        .collect();
    if valores.len() != 1 {
        return Err(format!("campo ausente ou duplicado ({nome}): {}", valores.len()).into());
    }
    Ok(valores[0])
}

fn executar_teste_irq_ipc(raiz: &Path, serial: File, erros: File, variante: &str) -> Resultado<()> {
    let reserva = UdpSocket::bind("127.0.0.1:17777")?;
    let mut comando = preparar_qemu_em(raiz, false, true, variante)?;
    comando
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    drop(reserva);
    let mut processo = ProcessoQemu(comando.spawn()?);
    let mut entrada = processo.0.stdin.take().ok_or("stdin indisponivel")?;
    let saida = processo.0.stdout.take().ok_or("stdout indisponivel")?;
    let stderr = processo.0.stderr.take().ok_or("stderr indisponivel")?;
    let (emissor, receptor) = mpsc::channel();
    let leitores = [
        leitor_saida(saida, true, emissor.clone()),
        leitor_saida(stderr, false, emissor),
    ];
    let mut captura = Captura {
        eventos: receptor,
        serial: Vec::new(),
        erros: Vec::new(),
        arquivo_serial: serial,
        arquivo_erros: erros,
    };
    let resultado = (|| -> Resultado<()> {
        captura.aguardar(
            &mut processo,
            "[teste] TERMINAL_PRONTO",
            0,
            Duration::from_secs(30),
        )?;
        aguardar_rede_geracao(&mut captura, &mut processo, 0, 1)?;
        captura.aguardar(
            &mut processo,
            "[supervisor] SERVICO_PRONTO",
            0,
            Duration::from_secs(30),
        )?;
        let inicio = enviar_comando(&mut entrada, &mut captura, "teste-irq-ipc\n")?;
        captura.aguardar_linha(
            &mut processo,
            "[teste-irq-ipc] CARGA_PRONTA CPL3",
            inicio,
            Duration::from_secs(10),
        )?;
        let resposta = "IRQIPC_0123456789_ABCDEFGHIJKLMNOPQRSTUVWXYZ";
        let inicio_eco = captura.serial.len();
        enviar_serial(&mut entrada, b"e")?;
        captura.aguardar(
            &mut processo,
            "[teste-irq-ipc] IRQ4 seq=",
            inicio_eco,
            Duration::from_secs(10),
        )?;
        entrada.write_all(b"co I")?;
        entrada.flush()?;
        captura.aguardar_linha(
            &mut processo,
            "[teste-irq-ipc] RETENCAO_FIM",
            inicio_eco,
            Duration::from_secs(10),
        )?;
        for bloco in b"RQIPC_0123456789_ABCDEFGHIJKLMNOPQRSTUVWXYZ\n".chunks(4) {
            entrada.write_all(bloco)?;
            entrada.flush()?;
            thread::sleep(Duration::from_millis(50));
        }
        captura.aguardar_linha(&mut processo, resposta, inicio_eco, Duration::from_secs(10))?;
        captura.drenar()?;
        let log = String::from_utf8_lossy(&captura.serial);
        validar_log_irq_ipc(&log)?;
        for falha in [
            "[teste-irq-ipc] FALHA_",
            "SERVICO_FALHOU",
            "SERVICO_FINALIZADO",
            "panic",
            "PANIC",
        ] {
            if log.contains(falha) {
                return Err(format!("Falha observada no ensaio IRQ/IPC: {falha}").into());
            }
        }
        Ok(())
    })();
    processo.encerrar();
    for leitor in leitores {
        let _ = leitor.join();
    }
    captura.drenar()?;
    resultado
}

fn validar_log_irq_ipc(log: &str) -> Resultado<()> {
    let mut sequencias = Vec::new();
    let mut sequencias_irq = Vec::new();
    let mut durante_trabalho_serial = 0usize;
    let mut irq4 = 0usize;
    let mut bytes_irq = 0usize;
    let mut primeira_irq = None;
    let mut primeiro_ipc_durante_irq = None;
    let mut retencao_fim = None;
    let mut deslocamento = 0usize;
    let mut pronto = false;
    for registro in log.split_inclusive('\n') {
        let Some(linha) = registro.strip_suffix('\n') else {
            if registro.starts_with("[teste-irq-ipc]") {
                return Err("Log IRQ/IPC terminou com um marcador incompleto".into());
            }
            deslocamento += registro.len();
            continue;
        };
        let linha = linha.strip_suffix('\r').unwrap_or(linha);
        if linha == "[teste-irq-ipc] CARGA_PRONTA CPL3" {
            pronto = true;
            deslocamento += registro.len();
            continue;
        }
        if !pronto {
            deslocamento += registro.len();
            continue;
        }
        if let Some(resto) = linha.strip_prefix("[teste-irq-ipc] IPC_DURANTE_TRABALHO_SERIAL seq=")
        {
            durante_trabalho_serial += 1;
            primeiro_ipc_durante_irq.get_or_insert(deslocamento);
            sequencias.push(resto.parse::<u64>()?);
        } else if let Some(resto) =
            linha.strip_prefix("[teste-irq-ipc] IPC_SEM_TRABALHO_SERIAL seq=")
        {
            sequencias.push(resto.parse::<u64>()?);
        } else if linha.starts_with("[teste-irq-ipc] IRQ4") {
            let resto = linha
                .strip_prefix("[teste-irq-ipc] IRQ4 seq=")
                .ok_or("Marcador IRQ4 malformado")?;
            let sequencia = resto.parse::<u64>()?;
            if sequencia == 0 || sequencias_irq.contains(&sequencia) {
                return Err("Sequência IRQ4 nula ou duplicada".into());
            }
            sequencias_irq.push(sequencia);
            irq4 += 1;
            primeira_irq.get_or_insert(deslocamento);
        } else if linha == "[teste-irq-ipc] RETENCAO_FIM" {
            retencao_fim = Some(deslocamento);
        } else if linha == "[teste-irq-ipc] BYTE_IRQ" && irq4 != 0 {
            bytes_irq += 1;
        }
        deslocamento += registro.len();
    }
    if !pronto {
        return Err("Marcador completo de prontidão ausente".into());
    }
    if sequencias != (1..=8).collect::<Vec<_>>() {
        return Err(
            format!("Sequência IPC ausente, duplicada ou fora de ordem: {sequencias:?}").into(),
        );
    }
    if durante_trabalho_serial == 0 || irq4 < 2 || bytes_irq < 8 {
        return Err(format!("Trabalho serial/IPC insuficiente: IPC durante trabalho serial={durante_trabalho_serial}, IRQ4={irq4}, bytes na janela={bytes_irq}").into());
    }
    if !matches!((primeira_irq, retencao_fim, primeiro_ipc_durante_irq), (Some(i), Some(r), Some(m)) if i < r && r < m)
    {
        return Err(
            "A ordem IRQ4, fim da retenção e IPC durante trabalho IRQ não foi observada".into(),
        );
    }
    Ok(())
}

fn testar_seletor_irq_ipc(raiz: &Path) -> Resultado<()> {
    let instante = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_nanos();
    let pasta = raiz.join(format!("evidencias/teste-seletor-irq-ipc-{instante}"));
    fs::create_dir(&pasta)?;
    let resultado = executar_teste_seletor_irq_ipc(
        raiz,
        File::create(pasta.join("serial.log"))?,
        File::create(pasta.join("qemu-stderr.log"))?,
    );
    fs::write(
        pasta.join("resultado.txt"),
        match &resultado {
            Ok(()) => "APROVADO: o seletor real observou IRQ4 e IPC elegíveis em duas chamadas consecutivas; escolheu IPC com preferir_irq=0 e IRQ com preferir_irq=1. A sequência IRQ foi confirmada pelo terminal e a segunda IPC permaneceu na FIFO até ser recebida. Isso demonstra somente esta sobreposição finita, não justiça sob fluxo ilimitado.".to_owned(),
            Err(e) => format!("FALHOU: {e}"),
        },
    )?;
    println!("Evidências: {}", pasta.display());
    resultado
}

fn executar_teste_seletor_irq_ipc(raiz: &Path, serial: File, erros: File) -> Resultado<()> {
    let reserva = UdpSocket::bind("127.0.0.1:17777")?;
    let mut comando = preparar_qemu_em(raiz, false, true, "teste-seletor-irq-ipc")?;
    comando
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    drop(reserva);
    let mut processo = ProcessoQemu(comando.spawn()?);
    let mut entrada = processo.0.stdin.take().ok_or("stdin indisponivel")?;
    let saida = processo.0.stdout.take().ok_or("stdout indisponivel")?;
    let stderr = processo.0.stderr.take().ok_or("stderr indisponivel")?;
    let (emissor, receptor) = mpsc::channel();
    let leitores = [
        leitor_saida(saida, true, emissor.clone()),
        leitor_saida(stderr, false, emissor),
    ];
    let mut captura = Captura {
        eventos: receptor,
        serial: Vec::new(),
        erros: Vec::new(),
        arquivo_serial: serial,
        arquivo_erros: erros,
    };
    let resultado = (|| -> Resultado<()> {
        captura.aguardar(
            &mut processo,
            "[teste] TERMINAL_PRONTO",
            0,
            Duration::from_secs(30),
        )?;
        aguardar_rede_geracao(&mut captura, &mut processo, 0, 1)?;
        captura.aguardar(
            &mut processo,
            "[supervisor] SERVICO_PRONTO",
            0,
            Duration::from_secs(30),
        )?;
        let inicio = enviar_comando(&mut entrada, &mut captura, "teste-seletor-irq-ipc\n")?;
        captura.aguardar_linha(
            &mut processo,
            "[teste-seletor-irq-ipc] CARGA_PRONTA CPL3",
            inicio,
            Duration::from_secs(10),
        )?;
        // O trigger A é um byte UART real. O terminal não chama receive depois
        // do pedido à rede até a sonda confirmar as duas fontes pendentes.
        let inicio_amostra = captura.serial.len();
        enviar_serial(&mut entrada, b"A")?;
        captura.aguardar_linha(
            &mut processo,
            "[teste-seletor-irq-ipc] FIM CPL3",
            inicio_amostra,
            Duration::from_secs(15),
        )?;
        // Drena eventos ainda enfileirados pelos leitores antes do oráculo.
        captura.drenar()?;
        let log = String::from_utf8_lossy(&captura.serial);
        validar_log_seletor_irq_ipc(&log)?;
        for falha in [
            "[teste-seletor-irq-ipc] FALHA_",
            "SERVICO_FALHOU",
            "SERVICO_FINALIZADO",
            "panic",
            "PANIC",
        ] {
            if log.contains(falha) {
                return Err(format!("Falha observada no ensaio IRQ/IPC: {falha}").into());
            }
        }
        Ok(())
    })();
    processo.encerrar();
    for leitor in leitores {
        let _ = leitor.join();
    }
    captura.drenar()?;
    resultado
}

fn validar_log_seletor_irq_ipc(log: &str) -> Resultado<()> {
    let mut pronto = None;
    let mut irq_pendente = None;
    let mut byte_lido = None;
    let mut ambas = None;
    let mut seletores = Vec::new();
    let mut primeira_ipc = None;
    let mut irq_confirmada = None;
    let mut ipc_restante = None;
    let mut fim = None;
    let mut deslocamento = 0usize;
    for registro in log.split_inclusive('\n') {
        let Some(linha) = registro.strip_suffix('\n') else {
            if registro.starts_with("[teste-seletor-irq-ipc]") {
                return Err("Log do seletor IRQ/IPC terminou com marcador incompleto".into());
            }
            deslocamento += registro.len();
            continue;
        };
        let linha = linha.strip_suffix('\r').unwrap_or(linha);
        if linha == "[teste-seletor-irq-ipc] CARGA_PRONTA CPL3" {
            if pronto.replace(deslocamento).is_some() {
                return Err("Marcador CARGA_PRONTA duplicado".into());
            }
            deslocamento += registro.len();
            continue;
        }
        if pronto.is_none() {
            deslocamento += registro.len();
            continue;
        }
        if linha.starts_with("[teste-seletor-irq-ipc] FALHA") {
            return Err(format!("Falha da fixture: {linha}").into());
        }
        if linha == "[teste-seletor-irq-ipc] IRQ4_PENDENTE mascara=1" {
            if irq_pendente.replace(deslocamento).is_some() {
                return Err("Marcador IRQ4_PENDENTE duplicado".into());
            }
        } else if linha == "[teste-seletor-irq-ipc] BYTE_A_LIDO" {
            if byte_lido.replace(deslocamento).is_some() {
                return Err("Marcador BYTE_A_LIDO duplicado".into());
            }
        } else if linha
            == "[teste-seletor-irq-ipc] AMBAS_PENDENTES irq=1 ipc=2 mascara=1 preferir_irq=0"
        {
            if ambas.replace(deslocamento).is_some() {
                return Err("Marcador AMBAS_PENDENTES duplicado".into());
            }
        } else if linha.starts_with("[teste-seletor-irq-ipc] SELETOR ") {
            seletores.push((deslocamento, linha.to_owned()));
        } else if linha.starts_with("[teste-seletor-irq-ipc] PRIMEIRA_IPC ") {
            if primeira_ipc
                .replace((deslocamento, linha.to_owned()))
                .is_some()
            {
                return Err("Marcador PRIMEIRA_IPC duplicado".into());
            }
        } else if let Some(resto) =
            linha.strip_prefix("[teste-seletor-irq-ipc] IRQ4_CONFIRMADA seq=")
        {
            if irq_confirmada
                .replace((deslocamento, resto.parse::<u64>()?))
                .is_some()
            {
                return Err("Marcador IRQ4_CONFIRMADA duplicado".into());
            }
        } else if linha.starts_with("[teste-seletor-irq-ipc] IPC_RESTANTE ") {
            if ipc_restante
                .replace((deslocamento, linha.to_owned()))
                .is_some()
            {
                return Err("Marcador IPC_RESTANTE duplicado".into());
            }
        } else if linha == "[teste-seletor-irq-ipc] FIM CPL3" {
            if fim.replace(deslocamento).is_some() {
                return Err("Marcador FIM duplicado".into());
            }
        } else if linha.starts_with("[teste-seletor-irq-ipc]") {
            return Err(format!("Marcador inesperado: {linha}").into());
        }
        deslocamento += registro.len();
    }
    let Some(pronto) = pronto else {
        return Err("Marcador completo de prontidão ausente".into());
    };
    let (
        Some(irq_pendente),
        Some(byte_lido),
        Some(ambas),
        Some((primeira_ipc_pos, primeira_ipc)),
        Some((confirmacao_pos, sequencia_irq)),
        Some((ipc_restante_pos, ipc_restante)),
        Some(fim),
    ) = (
        irq_pendente,
        byte_lido,
        ambas,
        primeira_ipc,
        irq_confirmada,
        ipc_restante,
        fim,
    )
    else {
        return Err("Faltam marcadores completos da amostra IRQ/IPC".into());
    };
    if seletores.len() != 2 {
        return Err(format!(
            "Esperados dois registros do seletor real, obtidos {}",
            seletores.len()
        )
        .into());
    }
    let primeira = &seletores[0].1;
    let segunda = &seletores[1].1;
    let origem_ipc = campo_hex_exato(&primeira_ipc, "origem=")?;
    let terminal_ref = campo_hex_exato(primeira, "terminal_ref=")?;
    let id_restante = campo_decimal_exato(&ipc_restante, "id=")?;
    let primeira_esperada = format!(
        "[teste-seletor-irq-ipc] SELETOR terminal_ref={terminal_ref:#x} preferir_irq=0 irq_pendente=1 irq_ref_atual=1 entregue=0 ipc_validas=2 escolhido=IPC origem={origem_ipc:#x} origem_atual=1 codigo=0x7e9 a=1"
    );
    let ipc_esperada =
        format!("[teste-seletor-irq-ipc] PRIMEIRA_IPC origem={origem_ipc:#x} codigo=0x7e9 id=1");
    let ipc_restante_esperada =
        format!("[teste-seletor-irq-ipc] IPC_RESTANTE origem={origem_ipc:#x} codigo=0x7e9 id=2");
    let segunda_esperada = format!(
        "[teste-seletor-irq-ipc] SELETOR terminal_ref={terminal_ref:#x} preferir_irq=1 irq_pendente=1 irq_ref_atual=1 entregue=0 ipc_validas=1 escolhido=IRQ seq={sequencia_irq}"
    );
    if primeira != &primeira_esperada
        || segunda != &segunda_esperada
        || primeira_ipc != ipc_esperada
        || ipc_restante != ipc_restante_esperada
        || terminal_ref == 0
        || sequencia_irq == 0
        || id_restante != 2
        || !(pronto < irq_pendente
            && irq_pendente < byte_lido
            && byte_lido < ambas
            && ambas < seletores[0].0
            && seletores[0].0 < primeira_ipc_pos
            && primeira_ipc_pos < seletores[1].0
            && seletores[1].0 < confirmacao_pos
            && confirmacao_pos < ipc_restante_pos
            && ipc_restante_pos < fim)
    {
        return Err("Ordem, origem, payload ou prioridades do seletor não conferem".into());
    }
    Ok(())
}

fn testar_irq_fifo_cheia(raiz: &Path) -> Resultado<()> {
    let instante = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_nanos();
    let pasta = raiz.join(format!("evidencias/teste-irq-fifo-cheia-{instante}"));
    fs::create_dir(&pasta)?;
    let serial = File::create(pasta.join("serial.log"))?;
    let erros = File::create(pasta.join("qemu-stderr.log"))?;
    let resultado = executar_teste_irq_fifo_cheia(raiz, serial, erros);
    let relatorio = match &resultado {
        Ok(()) => "# Ensaio T03-11 — IRQ4 com FIFO IPC cheia\n\nImagem opt-in; QEMU/TCG, pc/i440fx, 1 CPU, 256 MiB, OVMF e e1000.\n\nResultado: **APROVADO**. A sonda observou 15 mensagens da rede e uma do supervisor na caixa real do terminal, em ordem e com o slot IRQ inicialmente vazio. Após esse estado, o host enviou um byte UART real; a sonda observou IRQ4 selecionável, mascarada no PIC, enquanto a FIFO ainda continha as mesmas 16 mensagens. O seletor do kernel entregou a notificação IRQ e as 16 IPCs; o terminal validou cada origem, código e payload, confirmou a sequência e a sonda final confirmou a FIFO vazia.\n\nLimite: a evidência demonstra a reserva/coexistência e drenagem desta composição finita; não prova justiça sob carga ilimitada, tempestades UART nem hardware físico. Logs: `serial.log` e `qemu-stderr.log`.\n".to_owned(),
        Err(erro) => format!("# Ensaio T03-11 — IRQ4 com FIFO IPC cheia\n\nImagem opt-in; QEMU/TCG, pc/i440fx, 1 CPU, 256 MiB, OVMF e e1000.\n\nResultado: **FALHOU**. `{erro}`\n\nLogs: `serial.log` e `qemu-stderr.log`.\n"),
    };
    fs::write(pasta.join("resultado.txt"), relatorio)?;
    println!("Evidências: {}", pasta.display());
    resultado
}

fn executar_teste_irq_fifo_cheia(raiz: &Path, serial: File, erros: File) -> Resultado<()> {
    let reserva = UdpSocket::bind("127.0.0.1:17777")?;
    let mut comando = preparar_qemu_em(raiz, false, true, "teste-irq-fifo-cheia")?;
    comando
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    drop(reserva);
    let mut processo = ProcessoQemu(comando.spawn()?);
    let mut entrada = processo.0.stdin.take().ok_or("stdin indisponivel")?;
    let saida = processo.0.stdout.take().ok_or("stdout indisponivel")?;
    let stderr = processo.0.stderr.take().ok_or("stderr indisponivel")?;
    let (emissor, receptor) = mpsc::channel();
    let leitores = [
        leitor_saida(saida, true, emissor.clone()),
        leitor_saida(stderr, false, emissor),
    ];
    let mut captura = Captura {
        eventos: receptor,
        serial: Vec::new(),
        erros: Vec::new(),
        arquivo_serial: serial,
        arquivo_erros: erros,
    };
    let resultado = (|| -> Resultado<()> {
        captura.aguardar(
            &mut processo,
            "[teste] TERMINAL_PRONTO",
            0,
            Duration::from_secs(30),
        )?;
        aguardar_rede_geracao(&mut captura, &mut processo, 0, 1)?;
        captura.aguardar(
            &mut processo,
            "[supervisor] SERVICO_PRONTO",
            0,
            Duration::from_secs(30),
        )?;
        let inicio = enviar_comando(&mut entrada, &mut captura, "teste-irq-fifo-cheia\n")?;
        captura.aguardar_linha(
            &mut processo,
            "[teste-irq-fifo-cheia] FIFO_CHEIA quantidade=16 rede=15 supervisor=1 irq_vazia=1 imr_irq4=0 CPL3",
            inicio,
            Duration::from_secs(20),
        )?;
        // Estímulo real só depois da sonda atestar composição/fila cheia.
        let inicio_irq = captura.serial.len();
        enviar_serial(&mut entrada, b"A")?;
        captura.aguardar_linha(
            &mut processo,
            "[teste-irq-fifo-cheia] FIM CPL3",
            inicio_irq,
            Duration::from_secs(20),
        )?;
        captura.drenar()?;
        validar_log_irq_fifo_cheia(&captura.serial)?;
        let log = String::from_utf8_lossy(&captura.serial);
        if [
            "[teste-irq-fifo-cheia] FALHA_",
            "SERVICO_FALHOU",
            "SERVICO_FINALIZADO",
            "panic",
            "PANIC",
        ]
        .iter()
        .any(|marcador| log.contains(marcador))
        {
            return Err("Marcador de falha/morte/panic observado no ensaio T03-11".into());
        }
        Ok(())
    })();
    processo.encerrar();
    for leitor in leitores {
        let _ = leitor.join();
    }
    captura.drenar()?;
    resultado
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct EscolhaIrqFila {
    preferir_irq: u64,
    ipc_validas: u64,
    fila: u64,
    irq_pendente: u64,
    irq: bool,
    origem: u64,
    codigo: u64,
    a: u64,
    sequencia: u64,
}

fn parse_escolha_irq_fifo_cheia(linha: &str) -> Resultado<EscolhaIrqFila> {
    let tokens: Vec<&str> = linha.split_whitespace().collect();
    if tokens.len() < 2 || tokens[0] != "[teste-irq-fifo-cheia]" || tokens[1] != "SELETOR" {
        return Err("Trace do seletor T03-11 inválido".into());
    }
    let terminal = campo_hex(linha, "terminal_ref=")?;
    let preferir_irq = campo_decimal(linha, "preferir_irq=")?;
    let ipc_validas = campo_decimal(linha, "ipc_validas=")?;
    let fila = campo_decimal(linha, "fila=")?;
    let irq_pendente = campo_decimal(linha, "irq_pendente=")?;
    if terminal != 0x1_0000_0001 || preferir_irq > 1 || irq_pendente > 1 || ipc_validas != fila {
        return Err("Campos de identidade/estado no trace T03-11 inválidos".into());
    }
    let escolhido = tokens.get(7).copied().unwrap_or("");
    match escolhido {
        "escolhido=IRQ" if tokens.len() == 9 => {
            if !tokens[8].starts_with("seq=") {
                return Err("Sequência ausente no trace IRQ".into());
            }
            Ok(EscolhaIrqFila {
                preferir_irq,
                ipc_validas,
                fila,
                irq_pendente,
                irq: true,
                origem: 0,
                codigo: 0,
                a: 0,
                sequencia: campo_decimal(linha, "seq=")?,
            })
        }
        "escolhido=IPC" if tokens.len() == 11 => Ok(EscolhaIrqFila {
            preferir_irq,
            ipc_validas,
            fila,
            irq_pendente,
            irq: false,
            origem: campo_hex(linha, "origem=")?,
            codigo: campo_hex(linha, "codigo=")?,
            a: campo_decimal(linha, "a=")?,
            sequencia: 0,
        }),
        _ => Err("Tipo/campos da seleção do kernel inválidos".into()),
    }
}

fn validar_log_irq_fifo_cheia(serial: &[u8]) -> Resultado<()> {
    let serial = String::from_utf8_lossy(serial);
    let mut linhas = Vec::new();
    for registro in serial.split_inclusive('\n') {
        let Some(linha) = registro.strip_suffix('\n') else {
            let fragmento = registro.strip_prefix("raiz> ").unwrap_or(registro);
            if fragmento.starts_with("[teste-irq-fifo-cheia]") {
                return Err("Log T03-11 terminou com marcador incompleto".into());
            }
            continue;
        };
        let linha = linha.strip_suffix('\r').unwrap_or(linha);
        let linha = if let Some(depois_prompt) = linha.strip_prefix("raiz> ") {
            if depois_prompt.starts_with("[teste-irq-fifo-cheia]") {
                depois_prompt
            } else {
                linha
            }
        } else {
            linha
        };
        if linha.starts_with("[teste-irq-fifo-cheia]") {
            linhas.push(linha.to_owned());
        }
    }
    let marcador_rede =
        "[teste-irq-fifo-cheia] REDE_FIFO quantidade=15 origem=0x100000002 ordem=1..15 irq_vazia=1";
    let marcador_cheia = "[teste-irq-fifo-cheia] FIFO_CHEIA quantidade=16 rede=15 supervisor=1 irq_vazia=1 imr_irq4=0 CPL3";
    let marcador_irq = "[teste-irq-fifo-cheia] IRQ_PENDENTE fifo=16 irq_selecionavel=1 imr_irq4=1";
    let marcador_byte = "[teste-irq-fifo-cheia] BYTE_UART valor=0x41";
    let marcador_dreno_prefixo =
        "[teste-irq-fifo-cheia] DRENO_OK rede=15 supervisor=1 fifo=0 irq_seq=";
    let marcadores_fixos = [
        marcador_rede,
        marcador_cheia,
        marcador_irq,
        marcador_byte,
        "[teste-irq-fifo-cheia] FIM CPL3",
    ];
    let mut posicoes = [None; 5];
    let mut seletores = Vec::new();
    let mut irq_entregue = None;
    let mut dreno = None;
    for (indice, linha) in linhas.iter().enumerate() {
        if linha.starts_with("[teste-irq-fifo-cheia] FALHA") {
            return Err(format!("Falha da fixture T03-11: {linha}").into());
        }
        if let Some(resto) = linha.strip_prefix("[teste-irq-fifo-cheia] IRQ_ENTREGUE seq=") {
            let seq = decimal_evento_exato(resto)?;
            if seq == 0 || irq_entregue.replace((indice, seq)).is_some() {
                return Err("Marcador IRQ_ENTREGUE duplicado ou sequência nula".into());
            }
        } else if linha.starts_with("[teste-irq-fifo-cheia] SELETOR ") {
            seletores.push((indice, parse_escolha_irq_fifo_cheia(linha)?));
        } else if linha.starts_with(marcador_dreno_prefixo) {
            let esperado = format!(
                "{marcador_dreno_prefixo}{} byte=0x41",
                irq_entregue.map(|(_, seq)| seq).unwrap_or(0)
            );
            if irq_entregue.is_none()
                || linha.as_str() != esperado
                || dreno.replace(indice).is_some()
            {
                return Err(
                    "Marcador DRENO_OK não correlaciona sequência/byte ou está duplicado".into(),
                );
            }
        } else if linha.starts_with("[teste-irq-fifo-cheia]") {
            if let Some(i) = marcadores_fixos
                .iter()
                .position(|esperado| linha == *esperado)
            {
                if posicoes[i].replace(indice).is_some() {
                    return Err(format!("Marcador T03-11 duplicado: {linha}").into());
                }
            } else {
                return Err(format!("Marcador desconhecido/malformado T03-11: {linha}").into());
            }
        }
    }
    if posicoes.iter().any(Option::is_none) || irq_entregue.is_none() || dreno.is_none() {
        return Err("Faltam marcadores completos T03-11".into());
    }
    if !(posicoes[0] < posicoes[1]
        && posicoes[1] < posicoes[2]
        && posicoes[2] < posicoes[3]
        && posicoes[3] < dreno
        && dreno < posicoes[4])
    {
        return Err("Ordem dos marcadores de preparação, IRQ e drenagem inválida".into());
    }
    if seletores.len() != 17 {
        return Err(format!(
            "Esperadas 17 escolhas reais do seletor; observadas {}",
            seletores.len()
        )
        .into());
    }
    let (pos_irq_entregue, seq) = irq_entregue.unwrap();
    let primeira = seletores[0].1;
    if primeira.fila != 16 || primeira.ipc_validas != 16 || primeira.irq_pendente != 1 {
        return Err(
            "Seletor não observou simultaneamente IRQ4 e FIFO cheia na primeira escolha".into(),
        );
    }
    let mut indice_ipc = 1u64;
    let indice_trace = if primeira.preferir_irq == 0 {
        if primeira.irq
            || primeira.origem != 0x1_0000_0002
            || primeira.codigo != 0x7da
            || primeira.a != 1
        {
            return Err(
                "Preferência IPC inicial não selecionou a primeira mensagem da rede".into(),
            );
        }
        indice_ipc += 1;
        let segunda = seletores[1].1;
        if segunda.preferir_irq != 1
            || !segunda.irq
            || segunda.fila != 15
            || segunda.ipc_validas != 15
            || segunda.irq_pendente != 1
            || segunda.sequencia != seq
        {
            return Err(
                "Após IPC inicial, preferência alternada não selecionou a IRQ pendente".into(),
            );
        }
        2
    } else {
        if !primeira.irq || primeira.sequencia != seq {
            return Err("Preferência IRQ inicial não selecionou a IRQ pendente".into());
        }
        if seletores[1].1.preferir_irq != 0
            || seletores[1].1.irq
            || seletores[1].1.origem != 0x1_0000_0002
            || seletores[1].1.codigo != 0x7da
            || seletores[1].1.a != 1
        {
            return Err("Depois da IRQ, o seletor não alternou para a primeira IPC".into());
        }
        1
    };
    let mut preferencia_esperada = 0;
    for (_, escolha) in seletores.iter().skip(indice_trace) {
        if escolha.irq_pendente != 0
            || escolha.irq
            || escolha.fila as u64 != (17 - indice_ipc)
            || escolha.preferir_irq != preferencia_esperada
        {
            return Err("Seleção IPC após a única IRQ não preservou contagem/FIFO".into());
        }
        let (origem, codigo, payload) = if indice_ipc <= 15 {
            (0x1_0000_0002, 0x7da, indice_ipc)
        } else {
            (0x1_0000_0003, 0x7db, 1)
        };
        if escolha.origem != origem || escolha.codigo != codigo || escolha.a != payload {
            return Err(format!("FIFO/ordem de IPC divergente no item {indice_ipc}").into());
        }
        indice_ipc += 1;
        preferencia_esperada = 1;
    }
    if indice_ipc != 17 {
        return Err("Trace não cobriu as 16 IPCs ordenadas".into());
    }
    let pos_trace_irq = seletores
        .iter()
        .find(|(_, escolha)| escolha.irq)
        .map(|(posicao, _)| *posicao)
        .ok_or("Trace do seletor não contém entrega IRQ")?;
    let pos_dreno = dreno.unwrap();
    if seletores.iter().any(|(posicao, _)| *posicao >= pos_dreno) {
        return Err("Trace de seleção apareceu depois de DRENO_OK".into());
    }
    if !(posicoes[2].unwrap() < posicoes[3].unwrap()
        && posicoes[3].unwrap() < pos_trace_irq
        && pos_trace_irq < pos_irq_entregue
        && pos_irq_entregue < pos_dreno)
    {
        return Err("IRQ_PENDENTE, byte, seleção, entrega CPL3 e drenagem fora de ordem".into());
    }
    Ok(())
}

fn decimal_evento_exato(valor: &str) -> Resultado<u64> {
    if valor.is_empty() || !valor.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err("Campo decimal do evento não é inteiro exato".into());
    }
    Ok(valor.parse()?)
}

fn campo_decimal_exato(linha: &str, nome: &str) -> Resultado<u64> {
    let mut encontrados = linha
        .split_ascii_whitespace()
        .filter_map(|campo| campo.strip_prefix(nome));
    let valor = encontrados
        .next()
        .ok_or_else(|| format!("Campo ausente: {nome}"))?;
    if encontrados.next().is_some() {
        return Err(format!("Campo duplicado: {nome}").into());
    }
    Ok(valor.parse()?)
}

fn campo_hex_exato(linha: &str, nome: &str) -> Resultado<u64> {
    let mut encontrados = linha
        .split_ascii_whitespace()
        .filter_map(|campo| campo.strip_prefix(nome));
    let valor = encontrados
        .next()
        .ok_or_else(|| format!("Campo ausente: {nome}"))?;
    if encontrados.next().is_some() {
        return Err(format!("Campo duplicado: {nome}").into());
    }
    let valor = valor
        .strip_prefix("0x")
        .ok_or_else(|| format!("Campo hexadecimal malformado: {nome}"))?;
    Ok(u64::from_str_radix(valor, 16)?)
}

const RX_ORCAMENTO_DATAGRAMAS: u16 = 17;
const RX_ORCAMENTO_MASCARA_TOTAL: u64 = (1 << 17) - 1;

fn testar_rx_orcamento(raiz: &Path) -> Resultado<()> {
    let instante = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_nanos();
    let pasta = raiz.join(format!("evidencias/teste-rx-orcamento-{instante}"));
    fs::create_dir(&pasta)?;
    let serial = File::create(pasta.join("serial.log"))?;
    let erros = File::create(pasta.join("qemu-stderr.log"))?;
    let resultado = executar_teste_rx_orcamento(raiz, serial, erros);
    let mut relatorio = String::from(
        "# Ensaio T03-22 — orçamento RX\n\nImagem opt-in; QEMU/TCG, pc/i440fx, 1 CPU, 256 MiB, OVMF e e1000. Este comando não reutiliza a imagem normal.\n\n",
    );
    match &resultado {
        Ok(()) => relatorio.push_str(
            "Resultado: **APROVADO**. Antes da primeira consulta, o driver observou DD ativo em 17 descritores RX consecutivos a partir de `indice_rx`. Sem outro consumidor RX enquanto a rede estava bloqueada, os estados DD permaneceram prontos até a consulta; ela consumiu 16 quadros válidos já pendentes. A máscara do primeiro lote contém quaisquer 16 IDs distintos de 0 a 16; a consulta seguinte processou o único ID restante, e a união das máscaras foi 0x1ffff. Os contadores de quadros válidos e as máscaras são evidência separada do bit DD. O marcador de retorno comprova somente que a syscall `CEDER` retornou; não mede qual processo executou nesse intervalo. Estes datagramas não geram respostas UDP.\n\n",
        ),
        Err(erro) => relatorio.push_str(&format!("Resultado: **FALHOU**. `{erro}`\n\n")),
    }
    relatorio.push_str(
        "Pré-condicionamento: o ensaio observa a resposta ao ping do gateway e confirma um eco UDP comum antes da amostra. O host envia todos os datagramas antes de enviar o comando de liberação IPC. O marcador ARMADA indica prontidão da fixture; não é um marcador do kernel de bloqueio. O ensaio cobre esta rajada controlada e não mede desempenho, pressão contínua, perda geral, ou o comportamento de hardware físico. Logs: `serial.log` e `qemu-stderr.log`.\n",
    );
    fs::write(pasta.join("resultado.txt"), relatorio)?;
    println!("Evidências: {}", pasta.display());
    resultado
}

fn executar_teste_rx_orcamento(raiz: &Path, serial: File, erros: File) -> Resultado<()> {
    let reserva_eco = UdpSocket::bind("127.0.0.1:17777")?;
    let reserva_teste = UdpSocket::bind("127.0.0.1:17778")?;
    let mut comando = preparar_qemu_em(raiz, false, true, "teste-rx-orcamento")?;
    comando
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    drop((reserva_eco, reserva_teste));
    let mut processo = ProcessoQemu(comando.spawn()?);
    let mut entrada = processo.0.stdin.take().ok_or("stdin indisponivel")?;
    let saida = processo.0.stdout.take().ok_or("stdout indisponivel")?;
    let stderr = processo.0.stderr.take().ok_or("stderr indisponivel")?;
    let (emissor, receptor) = mpsc::channel();
    let leitores = [
        leitor_saida(saida, true, emissor.clone()),
        leitor_saida(stderr, false, emissor),
    ];
    let mut captura = Captura {
        eventos: receptor,
        serial: Vec::new(),
        erros: Vec::new(),
        arquivo_serial: serial,
        arquivo_erros: erros,
    };
    let resultado = (|| -> Resultado<()> {
        captura.aguardar(
            &mut processo,
            "[teste] TERMINAL_PRONTO",
            0,
            Duration::from_secs(30),
        )?;
        aguardar_rede_geracao(&mut captura, &mut processo, 0, 1)?;

        // Resolver e confirmar o próximo salto antes do eco UDP de preflight.
        // Assim, a amostra não depende da primeira troca ARP do serviço.
        let inicio_ping = enviar_comando(&mut entrada, &mut captura, "ping 10.0.2.2\n")?;
        aguardar_evento(
            &mut captura,
            &mut processo,
            inicio_ping,
            "[rede] PING OK 10.0.2.2",
            &[],
        )?;

        // O eco também confirma o hostfwd UDP dedicado antes da amostra.
        let socket_eco = socket_eco()?;
        conferir_eco(&socket_eco, b"rx-orcamento-preflight")?;
        let socket_teste = UdpSocket::bind("127.0.0.1:0")?;
        socket_teste.connect("127.0.0.1:17778")?;
        socket_teste.set_write_timeout(Some(Duration::from_secs(3)))?;

        let inicio = enviar_comando(&mut entrada, &mut captura, "teste-rx-orcamento\n")?;
        captura.aguardar(
            &mut processo,
            "[teste-rx-orcamento] ARMADA quantidade=17",
            inicio,
            Duration::from_secs(10),
        )?;

        for sequencia in 0..RX_ORCAMENTO_DATAGRAMAS {
            let mut payload = [0u8; 8];
            payload[..6].copy_from_slice(b"RZ23RX");
            payload[6..].copy_from_slice(&sequencia.to_be_bytes());
            let enviados = socket_teste.send(&payload)?;
            conferir(
                enviados == payload.len(),
                "Datagrama de teste enviado parcialmente",
            )?;
        }

        // Digitar o comando só depois que todos os envios UDP foram aceitos pelo
        // host; o terminal então libera a espera IPC da fixture no convidado.
        enviar_comando(&mut entrada, &mut captura, "teste-rx-orcamento-liberar\n")?;
        captura.aguardar_condicao(
            &mut processo,
            "continuação ou falha do orçamento RX",
            inicio,
            Duration::from_secs(10),
            |bytes| {
                String::from_utf8_lossy(bytes)
                    .split_inclusive('\n')
                    .any(|linha| {
                        linha.ends_with('\n')
                            && (linha.starts_with("[teste-rx-orcamento] FALHA")
                                || linha.starts_with(
                                    "[teste-rx-orcamento] CONTINUOU descritores=1 teste=1 ",
                                ))
                    })
            },
        )?;
        captura.drenar()?;
        let log = String::from_utf8_lossy(&captura.serial);
        validar_log_rx_orcamento(&log)?;
        for falha in [
            "[teste-rx-orcamento] FALHA",
            "SERVICO_FALHOU",
            "SERVICO_FINALIZADO",
            "panic",
            "PANIC",
        ] {
            if log.contains(falha) {
                return Err(format!("Falha no ensaio RX: {falha}").into());
            }
        }
        Ok(())
    })();
    processo.encerrar();
    for leitor in leitores {
        let _ = leitor.join();
    }
    captura.drenar()?;
    resultado
}

fn validar_log_rx_orcamento(log: &str) -> Resultado<()> {
    let esperados = [
        "[teste-rx-orcamento] ARMADA quantidade=17",
        "[teste-rx-orcamento] DD_PRONTOS descritores=17",
        "[teste-rx-orcamento] LOTE descritores=16 teste=16 mascara=",
        "[teste-rx-orcamento] CEDER_RETORNOU",
        "[teste-rx-orcamento] CONTINUOU descritores=1 teste=1 mascara_nova=",
    ];
    let mut posicoes = [None; 5];
    let mut mascara_primeiro_lote = None;
    let mut mascara_restante = None;
    let mut deslocamento = 0usize;
    for registro in log.split_inclusive('\n') {
        let Some(linha) = registro.strip_suffix('\n') else {
            if registro.starts_with("[teste-rx-orcamento]") {
                return Err("Log RX terminou com um marcador incompleto".into());
            }
            deslocamento += registro.len();
            continue;
        };
        let linha = linha.strip_suffix('\r').unwrap_or(linha);
        if linha.starts_with("[teste-rx-orcamento] FALHA") {
            return Err(format!("Marcador de falha no log: {linha}").into());
        }
        if let Some(indice) = esperados.iter().enumerate().position(|(indice, esperado)| {
            linha == *esperado
                || (indice_armada(*esperado)
                    && linha.starts_with("[teste-rx-orcamento] ARMADA quantidade=17 "))
                || (indice == 1
                    && linha
                        .starts_with("[teste-rx-orcamento] DD_PRONTOS descritores=17 indice_rx="))
                || (indice == 2
                    && linha
                        .starts_with("[teste-rx-orcamento] LOTE descritores=16 teste=16 mascara="))
                || (indice == 4
                    && linha.starts_with(
                        "[teste-rx-orcamento] CONTINUOU descritores=1 teste=1 mascara_nova=",
                    ))
        }) {
            if posicoes[indice].replace(deslocamento).is_some() {
                return Err(format!("Marcador duplicado: {}", esperados[indice]).into());
            }
            if indice == 0
                && (campo_numerico(linha, "rx_base").is_none()
                    || campo_numerico(linha, "quadros_base").is_none()
                    || !linha.ends_with("CPL3"))
            {
                return Err("Marcador ARMADA sem contadores de base e CPL3".into());
            }
            if indice == 1
                && (campo_numerico(linha, "indice_rx").is_none_or(|indice| indice >= 32)
                    || linha.split_whitespace().count() != 4)
            {
                return Err("Marcador DD_PRONTOS sem um índice RX válido".into());
            }
            if indice == 2 {
                let mascara = campo_numerico(linha, "mascara");
                if linha.split_whitespace().count() != 6
                    || campo_numerico(linha, "descritores") != Some(16)
                    || campo_numerico(linha, "teste") != Some(16)
                    || campo_numerico(linha, "esgotado") != Some(1)
                    || mascara.is_none_or(|valor| {
                        valor & !RX_ORCAMENTO_MASCARA_TOTAL != 0 || valor.count_ones() != 16
                    })
                {
                    return Err(format!("Lote RX inválido ou máscara incompleta: {linha}").into());
                }
                mascara_primeiro_lote = mascara;
            }
            if indice == 4 {
                let mascara_nova = campo_numerico(linha, "mascara_nova");
                let mascara_total = campo_numerico(linha, "mascara_total");
                if linha.split_whitespace().count() != 7
                    || campo_numerico(linha, "descritores") != Some(1)
                    || campo_numerico(linha, "teste") != Some(1)
                    || campo_numerico(linha, "esgotado") != Some(0)
                    || mascara_nova.is_none_or(|valor| {
                        valor & !RX_ORCAMENTO_MASCARA_TOTAL != 0 || valor.count_ones() != 1
                    })
                    || mascara_total != Some(RX_ORCAMENTO_MASCARA_TOTAL)
                {
                    return Err(
                        format!("Continuação RX inválida ou máscara ausente: {linha}").into(),
                    );
                }
                mascara_restante = mascara_nova;
                if let (Some(primeiro), Some(restante)) = (mascara_primeiro_lote, mascara_nova) {
                    if primeiro & restante != 0 || primeiro | restante != RX_ORCAMENTO_MASCARA_TOTAL
                    {
                        return Err(format!(
                            "Máscaras RX duplicam ou omitem identificadores: primeiro={primeiro:#x} restante={restante:#x}"
                        )
                        .into());
                    }
                }
            }
        }
        deslocamento += registro.len();
    }
    if posicoes.iter().any(Option::is_none) {
        return Err("Faltam marcadores completos do orçamento RX".into());
    }
    if !(posicoes[0] < posicoes[1]
        && posicoes[1] < posicoes[2]
        && posicoes[2] < posicoes[3]
        && posicoes[3] < posicoes[4])
    {
        return Err("Ordem ARMADA, DD, lote cheio, retorno de CEDER e continuação inválida".into());
    }
    if mascara_primeiro_lote.is_none() || mascara_restante.is_none() {
        return Err("Máscaras RX ausentes do oráculo".into());
    }
    Ok(())
}

fn indice_armada(marcador: &str) -> bool {
    marcador.starts_with("[teste-rx-orcamento] ARMADA")
}

const RX_QUOTA_MAGIC: &[u8; 6] = b"RZ25QT";

fn testar_rx_quota(raiz: &Path) -> Resultado<()> {
    let instante = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_nanos();
    let pasta = raiz.join(format!("evidencias/teste-rx-quota-{instante}"));
    fs::create_dir(&pasta)?;
    let serial = File::create(pasta.join("serial.log"))?;
    let erros = File::create(pasta.join("qemu-stderr.log"))?;
    let mut estimulos = File::create(pasta.join("estimulo-host.log"))?;
    let resultado = executar_teste_rx_quota(raiz, serial, erros, &mut estimulos);
    let relatorio = match &resultado {
        Ok(()) => format!(
            "# Ensaio T03-12 — RX real e controle supervisor\n\nResultado: **APROVADO** no perfil QEMU/TCG, pc/i440fx, 1 CPU, e1000 e hostfwd UDP. O preflight validou ping, eco UDP comum e eco pelo encaminhamento dedicado 17779→7779. O host submeteu quadros numerados 1–18; a rede CPL3 reconheceu IDs/magic/ordem no driver e publicou IPCs associados aos IDs. O kernel observou a recusa real do ID 16 com `FILA_CHEIA` enquanto a FIFO ainda continha os 15 IPCs da rede, validando o limite por origem. Depois, a recusa do segundo controle do supervisor com 15 mensagens da rede mais uma do supervisor comprova a capacidade total desta composição. Após drenagem validada em ordem, o ID 16 foi reenviado e aceito; IDs 17–18 também avançaram. Snapshot final do driver: 18 quadros, máscara 0x3ffff, flag de sequência inválida igual a zero; o terminal conferiu 18 IPCs da rede e o controle da referência atual do supervisor.\n\nLimites: é um fluxo finito em ordem observada pelo driver; UDP não garante ordem, e qualquer desordem/ausência classifica a execução como falha/inconclusiva da fixture, não como falha da FIFO IPC. A evidência não demonstra tráfego ininterrupto, justiça sob carga ilimitada, ausência geral de starvation, perda zero em rede ou hardware físico. `estimulo-host.log` registra submissões locais após cada `send`; o recebimento real é validado separadamente pelo driver no convidado. Hash da imagem: {}. Logs: `serial.log`, `qemu-stderr.log`, `estimulo-host.log`.\n",
            sha256_arquivo(&raiz.join("saida/teste-rx-quota/raiz-os.img"))?
        ),
        Err(erro) => format!(
            "# Ensaio T03-12 — RX real e controle supervisor\n\nResultado: **FALHOU**. `{erro}`\n\nO log do host registra somente os datagramas submetidos ao stdin de rede local; não prova entrega ao convidado. Logs: `serial.log`, `qemu-stderr.log`, `estimulo-host.log`.\n"
        ),
    };
    fs::write(pasta.join("resultado.txt"), relatorio)?;
    println!("Evidências: {}", pasta.display());
    resultado
}

fn executar_teste_rx_quota(
    raiz: &Path,
    serial: File,
    erros: File,
    estimulos: &mut File,
) -> Resultado<()> {
    let reserva_eco = UdpSocket::bind("127.0.0.1:17777")?;
    let reserva_teste = UdpSocket::bind("127.0.0.1:17779")?;
    let mut comando = preparar_qemu_em(raiz, false, true, "teste-rx-quota")?;
    comando
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    drop((reserva_eco, reserva_teste));
    let mut processo = ProcessoQemu(comando.spawn()?);
    let mut entrada = processo.0.stdin.take().ok_or("stdin QEMU indisponível")?;
    let saida = processo.0.stdout.take().ok_or("stdout QEMU indisponível")?;
    let stderr = processo.0.stderr.take().ok_or("stderr QEMU indisponível")?;
    let (emissor, receptor) = mpsc::channel();
    let leitores = [
        leitor_saida(saida, true, emissor.clone()),
        leitor_saida(stderr, false, emissor),
    ];
    let mut captura = Captura {
        eventos: receptor,
        serial: Vec::new(),
        erros: Vec::new(),
        arquivo_serial: serial,
        arquivo_erros: erros,
    };
    let resultado = (|| -> Resultado<()> {
        captura.aguardar(
            &mut processo,
            "[teste] TERMINAL_PRONTO",
            0,
            Duration::from_secs(30),
        )?;
        aguardar_rede_geracao(&mut captura, &mut processo, 0, 1)?;
        captura.aguardar(
            &mut processo,
            "[supervisor] SERVICO_PRONTO",
            0,
            Duration::from_secs(30),
        )?;
        let inicio_ping = enviar_comando(&mut entrada, &mut captura, "ping 10.0.2.2\n")?;
        aguardar_evento(
            &mut captura,
            &mut processo,
            inicio_ping,
            "[rede] PING OK 10.0.2.2",
            &[],
        )?;
        conferir_eco(&socket_eco()?, b"rz025-rx-quota-preflight")?;

        let teste = UdpSocket::bind("127.0.0.1:0")?;
        teste.connect("127.0.0.1:17779")?;
        teste.set_read_timeout(Some(Duration::from_secs(3)))?;
        teste.set_write_timeout(Some(Duration::from_secs(3)))?;
        let preflight = datagrama_rx_quota(0);
        conferir(
            teste.send(&preflight)? == preflight.len(),
            "Preflight UDP parcial",
        )?;
        let mut recebido = [0u8; 16];
        let tamanho = teste.recv(&mut recebido)?;
        conferir(
            recebido[..tamanho] == preflight,
            "Hostfwd UDP dedicado não devolveu o payload de preflight",
        )?;
        writeln!(estimulos, "PREFLIGHT_RZ25QT seq=0 echo=confirmado")?;
        estimulos.flush()?;

        let inicio = enviar_comando(&mut entrada, &mut captura, "teste-rx-quota\n")?;
        captura.aguardar_linha(
            &mut processo,
            "[teste-rx-quota] ARMADA ids=1..18 rede=CPL3 supervisor=CPL3 quota=15",
            inicio,
            Duration::from_secs(10),
        )?;
        for id in 1..=16u16 {
            let payload = datagrama_rx_quota(id);
            conferir(
                teste.send(&payload)? == payload.len(),
                "Datagrama RX parcial",
            )?;
            writeln!(estimulos, "SUBMETIDO id={id} bytes=8")?;
            estimulos.flush()?;
        }
        captura.aguardar_condicao(
            &mut processo,
            "FIFO de quinze mensagens reais da rede",
            inicio,
            Duration::from_secs(15),
            |bytes| linha_rx_quota_completa(bytes, "[teste-rx-quota] FIFO_REDE_PRONTA"),
        )?;
        captura.aguardar_condicao(
            &mut processo,
            "recusa kernel do ID 16 por quota da origem rede",
            inicio,
            Duration::from_secs(15),
            |bytes| linha_rx_quota_completa(bytes, "[teste-rx-quota] FILA_CHEIA"),
        )?;
        enviar_comando(&mut entrada, &mut captura, "teste-rx-quota-controle\n")?;
        captura.aguardar_condicao(
            &mut processo,
            "FIFO com quota da rede e controle supervisor",
            inicio,
            Duration::from_secs(15),
            |bytes| linha_rx_quota_completa(bytes, "[teste-rx-quota] FIFO_CHEIA"),
        )?;
        captura.aguardar_linha(
            &mut processo,
            "[teste-rx-quota] RETRY_OK id=16 quadros_driver=16 mascara=0xffff retries=1 driver_invalido=0",
            inicio,
            Duration::from_secs(15),
        )?;
        captura.aguardar_linha(
            &mut processo,
            "[teste-rx-quota] CONTINUACAO_PRONTA ids=17..18",
            inicio,
            Duration::from_secs(15),
        )?;
        for id in 17..=18u16 {
            let payload = datagrama_rx_quota(id);
            conferir(
                teste.send(&payload)? == payload.len(),
                "Datagrama RX parcial",
            )?;
            writeln!(estimulos, "SUBMETIDO id={id} bytes=8")?;
            estimulos.flush()?;
        }
        captura.aguardar_linha(
            &mut processo,
            "[teste-rx-quota] RESULTADO rx=18 ipc_rede=18 controle_supervisor=1 fila_cheia=1 retry_id=16 ordem=1..18 driver_invalido=0 CPL3",
            inicio,
            Duration::from_secs(15),
        )?;
        captura.drenar()?;
        validar_log_rx_quota(&captura.serial)?;
        let texto = String::from_utf8_lossy(&captura.serial);
        for falha in [
            "[teste-rx-quota] FALHA",
            "[microkernel] FALHA_NUCLEO",
            "[microkernel] SERVICO_FALHOU",
            "[microkernel] SERVICO_FINALIZADO",
            "[microkernel] PANICO",
        ] {
            if texto.contains(falha) {
                return Err(format!("Falha observada: {falha}").into());
            }
        }
        Ok(())
    })();
    processo.encerrar();
    for leitor in leitores {
        let _ = leitor.join();
    }
    captura.drenar()?;
    resultado
}

fn datagrama_rx_quota(id: u16) -> [u8; 8] {
    let mut payload = [0u8; 8];
    payload[..6].copy_from_slice(RX_QUOTA_MAGIC);
    payload[6..].copy_from_slice(&id.to_be_bytes());
    payload
}

fn validar_log_rx_quota(bytes: &[u8]) -> Resultado<()> {
    const REDE: u64 = 0x100000002;
    const TERMINAL: u64 = 0x100000001;
    const PREFIXO: &str = "[teste-rx-quota]";
    let texto = std::str::from_utf8(bytes)?;
    let mut linhas = Vec::new();
    for registro in texto.split_inclusive('\n') {
        if !registro.ends_with('\n') {
            if registro.contains(PREFIXO) {
                return Err("Marcador T03-12 incompleto".into());
            }
            continue;
        }
        let linha_com_prompt = registro.trim_end_matches(['\r', '\n']);
        let linha = linha_com_prompt
            .strip_prefix("raiz> ")
            .unwrap_or(linha_com_prompt);
        if linha_com_prompt.contains(PREFIXO) && !linha.starts_with(PREFIXO) {
            return Err("Eco ou prefixo antes de marcador T03-12".into());
        }
        if linha.starts_with("[teste-rx-quota] FALHA")
            || linha.starts_with("[microkernel] FALHA_NUCLEO")
            || linha.starts_with("[microkernel] SERVICO_FALHOU")
            || linha.starts_with("[microkernel] SERVICO_FINALIZADO")
            || linha.starts_with("[microkernel] PANICO")
            || linha_com_prompt.to_ascii_lowercase().contains("panic")
        {
            return Err(format!("Falha observada no serial: {linha}").into());
        }
        if linha.starts_with(PREFIXO) {
            linhas.push(linha.to_owned());
        }
    }
    let esperado = [
        "[teste-rx-quota] ARMADA ids=1..18 rede=CPL3 supervisor=CPL3 quota=15",
        "[teste-rx-quota] FIFO_REDE_PRONTA quantidade=15 origem=0x100000002",
        "[teste-rx-quota] FIFO_CHEIA rede=15 supervisor=1 total=16 CPL3",
            "[teste-rx-quota] RETRY_OK id=16 quadros_driver=16 mascara=0xffff retries=1 driver_invalido=0",
        "[teste-rx-quota] CONTINUACAO_PRONTA ids=17..18",
            "[teste-rx-quota] RESULTADO rx=18 ipc_rede=18 controle_supervisor=1 fila_cheia=1 retry_id=16 ordem=1..18 driver_invalido=0 CPL3",
    ];
    let mut posicoes = [None; 6];
    let mut trace_fila = None;
    for (indice, linha) in linhas.iter().enumerate() {
        if linha.starts_with("[teste-rx-quota] FILA_CHEIA ") {
            if trace_fila.replace(indice).is_some() {
                return Err("Trace FILA_CHEIA duplicado".into());
            }
            let campos: Vec<&str> = linha.split_ascii_whitespace().collect();
            if campos.len() != 10
                || campo_hex_exato(linha, "origem=")? != REDE
                || campo_hex_exato(linha, "destino=")? != TERMINAL
                || campo_decimal_exato(linha, "id=")? != 16
                || campo_hex_exato(linha, "codigo=")? != 0x7ba
                || campo_hex_exato(linha, "retorno=")? != u64::MAX - 2
                || campo_decimal_exato(linha, "fila=")? != 15
                || campo_decimal_exato(linha, "quota_rede=")? != 15
                || campos[9] != "CPL3"
            {
                return Err(
                    "Trace de FILA_CHEIA não prova recusa do ID16 com 15 IPCs de rede".into(),
                );
            }
            continue;
        }
        if let Some((pos, esperado)) = esperado
            .iter()
            .enumerate()
            .find(|(_, marcador)| linha.starts_with(**marcador))
        {
            if posicoes[pos].replace(indice).is_some() {
                return Err(format!("Marcador T03-12 duplicado: {esperado}").into());
            }
            if linha != esperado {
                return Err(format!("Campos inesperados no marcador T03-12: {linha}").into());
            }
        } else {
            return Err(format!("Marcador T03-12 desconhecido/malformado: {linha}").into());
        }
    }
    if posicoes.iter().any(Option::is_none) || trace_fila.is_none() {
        return Err("Marcadores essenciais ou trace de quota ausentes".into());
    }
    let [Some(armada), Some(fifo_rede), Some(fifo_cheia), Some(retry), Some(continuacao), Some(fim)] =
        posicoes
    else {
        return Err("Posições essenciais T03-12 ausentes".into());
    };
    let trace = trace_fila.unwrap();
    if !(armada < fifo_rede
        && armada < trace
        && fifo_rede < fifo_cheia
        && trace < fifo_cheia
        && fifo_cheia < retry
        && retry < continuacao
        && continuacao < fim)
    {
        return Err("Ordem das barreiras/recusa/drenagem/retry T03-12 inválida".into());
    }
    Ok(())
}

fn testar_rz022(raiz: &Path) -> Resultado<()> {
    let instante = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_nanos();
    let pasta = raiz
        .join("evidencias")
        .join(format!("execucao-rz-022-{instante}"));
    fs::create_dir(&pasta)?;
    println!("Testando polling RX da rede, recuperação manual e backoff automático RZ-022...");
    let resultado_manual = executar_caso_rz022(raiz, &pasta, false);
    let resultado_auto = executar_caso_rz022(raiz, &pasta, true);
    let aprovado = resultado_manual.is_ok() && resultado_auto.is_ok();
    let mut relatorio = String::from(
        "# Teste integrado RZ-022\n\nImagem opt-in separada da imagem normal; QEMU/TCG, pc/i440fx, 1 CPU, 256 MiB, OVMF e e1000. Esta execução preserva os relatórios históricos.\n\n",
    );
    for (nome, resultado) in [
        ("Recuperação manual, FIFO de reset e RX", &resultado_manual),
        ("Retentativas automáticas com backoff", &resultado_auto),
    ] {
        relatorio.push_str(&format!(
            "## {nome}\n\nResultado: **{}**.\n\n",
            if resultado.is_ok() {
                "APROVADO"
            } else {
                "FALHOU"
            }
        ));
        match resultado {
            Ok(texto) => relatorio.push_str(texto),
            Err(erro) => relatorio.push_str(&format!("Falha: `{erro}`\n")),
        }
        relatorio.push('\n');
    }
    let caminho_resultado = pasta.join("resultado.txt");
    fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&caminho_resultado)?
        .write_all(relatorio.as_bytes())?;
    println!("Evidências: {}", pasta.display());
    if aprovado {
        Ok(())
    } else {
        Err(format!(
            "Teste RZ-022 falhou; consulte {}",
            caminho_resultado.display()
        )
        .into())
    }
}

fn executar_caso_rz022(raiz: &Path, pasta: &Path, automatico: bool) -> Resultado<String> {
    let nome = if automatico { "automatico" } else { "manual" };
    let arquivo_serial = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(pasta.join(format!("serial-{nome}.log")))?;
    let arquivo_erros = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(pasta.join(format!("qemu-{nome}-stderr.log")))?;
    let mut comando = preparar_qemu_em(raiz, false, true, "teste-rz022")?;
    comando
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut processo = ProcessoQemu(comando.spawn()?);
    let mut entrada = processo.0.stdin.take().ok_or("stdin QEMU indisponível")?;
    let saida = processo.0.stdout.take().ok_or("stdout QEMU indisponível")?;
    let erros = processo.0.stderr.take().ok_or("stderr QEMU indisponível")?;
    let (emissor, receptor) = mpsc::channel();
    let leitores = [
        leitor_saida(saida, true, emissor.clone()),
        leitor_saida(erros, false, emissor),
    ];
    let mut captura = Captura {
        eventos: receptor,
        serial: Vec::new(),
        erros: Vec::new(),
        arquivo_serial,
        arquivo_erros,
    };
    let resultado = (|| -> Resultado<String> {
        captura.aguardar(
            &mut processo,
            "[teste] TERMINAL_PRONTO",
            0,
            Duration::from_secs(30),
        )?;
        if !automatico {
            testar_fifo_reset_rz022(&mut entrada, &mut captura, &mut processo)?;
        }
        aguardar_evento(
            &mut captura,
            &mut processo,
            0,
            "[supervisor] SERVICO_PRONTO",
            &["CPL=3", "modo=manual", "limite=4"],
        )?;
        aguardar_rede_geracao(&mut captura, &mut processo, 0, 1)?;

        if !automatico {
            aguardar_fifo_ack_rz022(&mut captura, &mut processo)?;
            conferir_conectividade(
                &mut entrada,
                &mut captura,
                &mut processo,
                1,
                "após preservar IPC durante reset",
                &mut EvidenciasSmoke::default(),
            )?;
            testar_polling_rx_rz022(&mut entrada, &mut captura, &mut processo)?;
            let inicio = enviar_comando(&mut entrada, &mut captura, "falhar-rede\n")?;
            aguardar_evento(
                &mut captura,
                &mut processo,
                inicio,
                "[microkernel] SERVICO_FALHOU",
                &["id=2", "vetor=6"],
            )?;
            let consulta = enviar_comando(&mut entrada, &mut captura, "supervisao\n")?;
            aguardar_evento(
                &mut captura,
                &mut processo,
                consulta,
                "[supervisor] MODO manual",
                &["reinicios=0"],
            )?;
            let inicio = enviar_comando(&mut entrada, &mut captura, "reiniciar-rede\n")?;
            aguardar_reinicio(&mut captura, &mut processo, inicio, 1, 2, false)?;
            conferir_conectividade(
                &mut entrada,
                &mut captura,
                &mut processo,
                2,
                "após recuperação manual",
                &mut EvidenciasSmoke::default(),
            )?;
            return Ok("Após CTRL.RST, o helper real de inicialização guardou 16 IPCs; outros 16 permaneceram na FIFO do kernel, a 33ª tentativa recebeu FILA_CHEIA, e após o prazo observado de 500 tiques a nova rede confirmou os ACKs 1..32 em ordem. No ciclo normal, a rede anunciou seu próximo prazo de polling; o kernel registrou bloqueio por 10 tiques, o host enviou um datagrama após esse marcador, e o harness exigiu marcador de timeout antes do eco idêntico e do contador RX/UDP crescente. A ordem observada não prova em que instante o dispositivo recebeu o datagrama. UD2 da rede permaneceu sem reinício no modo manual; `reiniciar-rede` criou a geração 2, que respondeu a ping e UDP.\n".into());
        }

        let inicio = enviar_comando(&mut entrada, &mut captura, "supervisao auto\n")?;
        aguardar_evento(
            &mut captura,
            &mut processo,
            inicio,
            "[supervisor] MODO auto",
            &["limite=4"],
        )?;
        let linha_auto = esperar_marcador_rz022(
            &mut captura,
            &mut processo,
            inicio,
            "AUTO_SOLICITADO",
            "",
            Duration::from_secs(5),
        )?;
        let tique_auto = campo_numerico(&linha_auto, "tique")
            .ok_or("Tique do comando para ativar o modo automático ausente")?;
        let mut resumo = String::new();
        for tentativa in 1..=3u64 {
            let antiga = tentativa;
            let nova = tentativa + 1;
            let inicio = enviar_comando(&mut entrada, &mut captura, "falhar-rede\n")?;
            let solicitacao = esperar_marcador_rz022(
                &mut captura,
                &mut processo,
                inicio,
                "FALHA_SOLICITADA",
                &format!("referencia={:#x}", (antiga << 32) | 2),
                Duration::from_secs(5),
            )?;
            let tique_solicitacao = campo_numerico(&solicitacao, "tique")
                .ok_or("Tique da solicitação de falha ausente")?;
            aguardar_evento(
                &mut captura,
                &mut processo,
                inicio,
                "[microkernel] SERVICO_FALHOU",
                &["id=2", "vetor=6"],
            )?;
            let deadline = esperar_marcador_rz022(
                &mut captura,
                &mut processo,
                inicio,
                "DEADLINE",
                &format!("referencia={:#x}", (antiga << 32) | 2),
                Duration::from_secs(10),
            )?;
            conferir(
                campo_numerico(&deadline, "atraso")
                    == Some(match tentativa {
                        1 => 100,
                        2 => 200,
                        _ => 400,
                    }),
                "Atraso automático no log não segue 100/200/400 tiques",
            )?;
            let atraso = campo_numerico(&deadline, "atraso").ok_or("Atraso ausente")?;
            let inicio_tick = campo_numerico(&deadline, "inicio").ok_or("Tique inicial ausente")?;
            let evento_tentativa = esperar_marcador_rz022(
                &mut captura,
                &mut processo,
                inicio,
                "TENTATIVA",
                &format!("referencia={:#x}", (antiga << 32) | 2),
                Duration::from_secs(10),
            )?;
            conferir(
                campo_numerico(&evento_tentativa, "indice") == Some(tentativa),
                "Índice da tentativa automática inesperado",
            )?;
            let tentativa_tick =
                campo_numerico(&evento_tentativa, "tique").ok_or("Tique da tentativa ausente")?;
            conferir(
                tique_após_prazo_rz022(inicio_tick, tentativa_tick, atraso),
                "Tentativa automática ocorreu antes do prazo lógico",
            )?;
            verificar_esperas_auto_rz022(
                &captura.serial,
                (antiga << 32) | 2,
                tique_auto,
                tique_solicitacao,
                inicio_tick,
                tentativa_tick,
                atraso,
            )?;
            let reinicio = esperar_marcador_rz022(
                &mut captura,
                &mut processo,
                inicio,
                "REINICIADO",
                &format!("antiga={:#x}", (antiga << 32) | 2),
                Duration::from_secs(10),
            )?;
            conferir(
                campo_numerico(&reinicio, "nova") == Some((nova << 32) | 2)
                    && campo_numerico(&reinicio, "tique")
                        .is_some_and(|t| tique_após_prazo_rz022(inicio_tick, t, atraso)),
                "Referência/tique do reinício não respeita a identidade e o prazo registrados",
            )?;
            aguardar_rede_geracao(&mut captura, &mut processo, inicio, nova)?;
            conferir_conectividade(
                &mut entrada,
                &mut captura,
                &mut processo,
                nova,
                "após recuperação automática",
                &mut EvidenciasSmoke::default(),
            )?;
            resumo.push_str(&format!("Tentativa {tentativa}: referência {:#x}, VIDA_DESPERTA correlacionada com a falha e espera FINITO registrada entre deadline e tentativa; backoff de {atraso} tiques (diferença observada: {}), geração {nova} respondeu a ping/UDP.\n", (antiga << 32) | 2, tentativa_tick.wrapping_sub(inicio_tick)));
        }
        conferir(
            conferir_arenas_do_log(&captura.serial, 3)?.len() == 4,
            "Cenário automático não preservou as quatro encarnações esperadas",
        )?;
        Ok(resumo)
    })();
    drop(entrada);
    processo.encerrar();
    let mut erro_finalizacao = None;
    for leitor in leitores {
        if leitor.join().is_err() {
            erro_finalizacao = Some("Falha na thread de captura de QEMU".into());
        }
    }
    if let Err(erro) = captura.drenar() {
        erro_finalizacao = Some(erro.to_string());
    }
    captura.arquivo_serial.flush()?;
    captura.arquivo_erros.flush()?;
    match resultado {
        Err(erro) => Err(erro),
        Ok(resumo) => match erro_finalizacao {
            Some(erro) => Err(erro.into()),
            None => Ok(resumo),
        },
    }
}

fn testar_polling_rx_rz022(
    entrada: &mut std::process::ChildStdin,
    captura: &mut Captura,
    processo: &mut ProcessoQemu,
) -> Resultado<()> {
    let inicio = enviar_comando(entrada, captura, "teste-rz022-rede\n")?;
    let antes = esperar_marcador_rz022(
        captura,
        processo,
        inicio,
        "ANTES_ESPERA",
        "prazo=10",
        Duration::from_secs(10),
    )?;
    let recebidos_antes =
        campo_numerico(&antes, "recebidos").ok_or("Contador RX ausente antes da espera")?;
    let tique_antes = campo_numerico(&antes, "tique").ok_or("Tique ausente antes da espera")?;
    aguardar_evento(
        captura,
        processo,
        inicio,
        "[teste-rz022] REDE_BLOQUEADA",
        &["referencia=0x100000002", "prazo=10"],
    )?;
    let socket = socket_eco()?;
    let payload = b"rz022-rx-apres-espera";
    conferir(
        socket.send(payload)? == payload.len(),
        "Envio UDP incompleto",
    )?;
    aguardar_evento(
        captura,
        processo,
        inicio,
        "[teste-rz022] REDE_TIMEOUT",
        &["referencia=0x100000002", "prazo=10"],
    )?;
    let log = String::from_utf8_lossy(&captura.serial);
    let pos_bloqueio = log
        .find("[teste-rz022] REDE_BLOQUEADA referencia=0x100000002 modo=FINITO prazo=10")
        .ok_or("Marcador de bloqueio da rede ausente")?;
    let pos_timeout = log
        .find("[teste-rz022] REDE_TIMEOUT referencia=0x100000002 prazo=10")
        .ok_or("Marcador de timeout da rede ausente")?;
    conferir(
        pos_bloqueio < pos_timeout,
        "Timeout da rede apareceu antes do marcador de bloqueio",
    )?;
    let mut resposta = [0u8; 65_536];
    let tamanho = socket.recv(&mut resposta)?;
    conferir(
        &resposta[..tamanho] == payload,
        "Resposta UDP diferente do payload enviado",
    )?;
    let depois = esperar_marcador_rz022(
        captura,
        processo,
        inicio,
        "RX_PROGREDIU",
        "",
        Duration::from_secs(10),
    )?;
    let recebidos_depois =
        campo_numerico(&depois, "recebidos").ok_or("Contador RX ausente após a espera")?;
    let ecos = campo_numerico(&depois, "ecos").ok_or("Contador UDP ausente após a espera")?;
    let tique_depois = campo_numerico(&depois, "tique").ok_or("Tique ausente após a espera")?;
    conferir(
        recebidos_depois > recebidos_antes && ecos > 0,
        "RX/eco UDP não progrediu após o anúncio do próximo prazo de polling",
    )?;
    conferir(
        tique_depois.wrapping_sub(tique_antes) < (1u64 << 63),
        "Contador de tiques retrocedeu fora da comparação modular",
    )?;
    conferir(
        captura.serial.len() > inicio,
        "Nenhuma saída serial foi capturada no ensaio RX",
    )?;
    Ok(())
}

fn testar_fifo_reset_rz022(
    entrada: &mut std::process::ChildStdin,
    captura: &mut Captura,
    processo: &mut ProcessoQemu,
) -> Resultado<()> {
    const REFERENCIA_INICIAL: u64 = 0x1_0000_0002;
    let janela = esperar_marcador_rz022(
        captura,
        processo,
        0,
        "FIFO_JANELA",
        &format!("referencia={REFERENCIA_INICIAL:#x}"),
        Duration::from_secs(30),
    )?;
    let inicio_janela =
        campo_numerico(&janela, "inicio").ok_or("Tique inicial da janela ausente")?;
    conferir(
        campo_numerico(&janela, "tiques") == Some(500)
            && campo_numerico(&janela, "inicio").is_some(),
        "Janela do reset não declarou referência e duração esperadas",
    )?;
    let inicio = enviar_comando(entrada, captura, "teste-rz022-fifo-iniciar\n")?;
    aguardar_evento(
        captura,
        processo,
        inicio,
        "[teste-rz022-rede] FIFO_PRIMEIRA_ENVIADA",
        &["quantidade=16"],
    )?;
    let stash = esperar_marcador_rz022(
        captura,
        processo,
        inicio,
        "FIFO_STASH_CHEIO",
        &format!("referencia={REFERENCIA_INICIAL:#x}"),
        Duration::from_secs(10),
    )?;
    conferir(
        campo_numerico(&stash, "quantidade") == Some(16),
        "Stash de IPC durante reset não confirmou 16 mensagens",
    )?;
    let tick_stash = campo_numerico(&stash, "tique").ok_or("Tique do stash cheio ausente")?;
    conferir(
        tique_após_prazo_rz022(inicio_janela, tick_stash, 1)
            && tick_stash.wrapping_sub(inicio_janela) < 500,
        "Stash encheu fora da janela inicial do reset",
    )?;
    let inicio = enviar_comando(entrada, captura, "teste-rz022-fifo-continuar\n")?;
    aguardar_evento(
        captura,
        processo,
        inicio,
        "[teste-rz022-rede] FILA_CHEIA_CONFIRMADA",
        &["seq=33"],
    )?;
    let fim = esperar_marcador_rz022(
        captura,
        processo,
        inicio,
        "FIFO_JANELA_FIM",
        &format!("referencia={REFERENCIA_INICIAL:#x}"),
        Duration::from_secs(15),
    )?;
    let fim_janela = campo_numerico(&fim, "tiques").ok_or("Tique final da janela ausente")?;
    conferir(
        tique_após_prazo_rz022(inicio_janela, fim_janela, 500),
        "Janela de reset fechou antes de decorrerem 500 tiques observados",
    )?;
    Ok(())
}

fn aguardar_fifo_ack_rz022(captura: &mut Captura, processo: &mut ProcessoQemu) -> Resultado<()> {
    captura.aguardar_condicao(
        processo,
        "32 ACKs FIFO ordenados",
        0,
        Duration::from_secs(10),
        |bytes| {
            sequencias_fifo_rz022(bytes)
                .is_some_and(|sequencias| sequencias == (1..=32).collect::<Vec<_>>())
        },
    )?;
    conferir(
        sequencias_fifo_rz022(&captura.serial).as_deref() == Some(&(1..=32).collect::<Vec<_>>()),
        "ACKs pós-reset não confirmaram a FIFO completa em ordem 1..32",
    )
}

fn sequencias_fifo_rz022(bytes: &[u8]) -> Option<Vec<u64>> {
    let mut sequencias = Vec::new();
    for linha in String::from_utf8_lossy(bytes).split_inclusive('\n') {
        if !linha.ends_with('\n') || !linha.starts_with("[teste-rz022-rede] FIFO_ACK ") {
            continue;
        }
        let seq = campo_numerico(linha, "seq")?;
        let referencia = campo_numerico(linha, "referencia")?;
        let tique = campo_numerico(linha, "tique")?;
        if referencia != 0x1_0000_0002 || tique == 0 {
            return None;
        }
        sequencias.push(seq);
    }
    Some(sequencias)
}

fn esperar_marcador_rz022(
    captura: &mut Captura,
    processo: &mut ProcessoQemu,
    desde: usize,
    marcador: &str,
    campo: &str,
    prazo: Duration,
) -> Resultado<String> {
    let prefixo = format!(
        "[teste-rz022-{}] {marcador}",
        if matches!(
            marcador,
            "DEADLINE" | "TENTATIVA" | "REINICIADO" | "AUTO_SOLICITADO" | "FALHA_SOLICITADA"
        ) {
            "supervisor"
        } else {
            "rede"
        }
    );
    captura.aguardar_condicao(processo, &prefixo, desde, prazo, |bytes| {
        String::from_utf8_lossy(bytes)
            .split_inclusive('\n')
            .any(|linha| {
                linha.ends_with('\n')
                    && linha.trim_end_matches(['\r', '\n']).starts_with(&prefixo)
                    && (campo.is_empty() || linha.split_whitespace().any(|c| c == campo))
            })
    })?;
    String::from_utf8_lossy(&captura.serial[desde..])
        .split_inclusive('\n')
        .find(|linha| {
            linha.ends_with('\n')
                && linha.trim_end_matches(['\r', '\n']).starts_with(&prefixo)
                && (campo.is_empty() || linha.split_whitespace().any(|c| c == campo))
        })
        .map(str::to_owned)
        .ok_or_else(|| format!("Marcador RZ-022 sem linha completa: {prefixo}").into())
}

fn verificar_esperas_auto_rz022(
    serial: &[u8],
    referencia: u64,
    tique_auto: u64,
    tique_solicitacao: u64,
    tique_deadline: u64,
    tique_tentativa: u64,
    atraso: u64,
) -> Resultado<()> {
    let texto = String::from_utf8_lossy(serial);
    let linhas: Vec<&str> = texto
        .split_inclusive('\n')
        .filter(|linha| linha.ends_with('\n'))
        .map(|linha| {
            linha
                .trim_end_matches(['\r', '\n'])
                .strip_prefix("raiz> ")
                .unwrap_or_else(|| linha.trim_end_matches(['\r', '\n']))
        })
        .collect();
    let ref_txt = format!("referencia={referencia:#x}");
    let observador_txt = "observador=0x100000003";

    let falha = linhas
        .iter()
        .filter(|linha| {
            linha.starts_with("[teste-rz022-supervisor] FALHA_SOLICITADA")
                && linha
                    .split_whitespace()
                    .any(|campo| campo == ref_txt.as_str())
        })
        .filter_map(|linha| campo_numerico(linha, "tique"))
        .find(|tique| *tique == tique_solicitacao)
        .ok_or("Marcador de solicitação de falha não correlaciona com o tique esperado")?;

    let desperta = linhas
        .iter()
        .filter(|linha| {
            linha.starts_with("[teste-rz022] VIDA_DESPERTA")
                && linha
                    .split_whitespace()
                    .any(|campo| campo == observador_txt)
                && linha
                    .split_whitespace()
                    .any(|campo| campo == ref_txt.as_str())
                && campo_numerico(linha, "tipo") == Some(1)
                && campo_numerico(linha, "estado") == Some(2)
        })
        .filter_map(|linha| campo_numerico(linha, "tique"))
        .find(|tique| tique_após_prazo_rz022(falha, *tique, 0))
        .ok_or_else(|| format!("Falta VIDA_DESPERTA de falha para a referência {referencia:#x}"))?;

    if referencia == 0x100000002 {
        let sem_prazo = linhas
            .iter()
            .filter(|linha| {
                linha.starts_with("[teste-rz022] EVENTO_BLOQUEADO")
                    && linha
                        .split_whitespace()
                        .any(|campo| campo == observador_txt)
                    && linha
                        .split_whitespace()
                        .any(|campo| campo == ref_txt.as_str())
                    && linha
                        .split_whitespace()
                        .any(|campo| campo == "modo=SEM_PRAZO")
            })
            .filter_map(|linha| campo_numerico(linha, "inicio"))
            .find(|tique| {
                tique_após_prazo_rz022(tique_auto, *tique, 0)
                    && tique_após_prazo_rz022(*tique, falha, 0)
            });
        conferir(
            sem_prazo.is_some(),
            "Supervisor não registrou espera SEM_PRAZO após o comando auto e antes da falha",
        )?;
    }

    conferir(
        tique_após_prazo_rz022(tique_solicitacao, desperta, 0)
            && tique_após_prazo_rz022(desperta, tique_deadline, 0),
        "Despertar de vida não corresponde à falha antes do início do backoff",
    )?;

    let finito = linhas.iter().any(|linha| {
        linha.starts_with("[teste-rz022] EVENTO_BLOQUEADO")
            && linha
                .split_whitespace()
                .any(|campo| campo == observador_txt)
            && linha
                .split_whitespace()
                .any(|campo| campo == ref_txt.as_str())
            && linha.split_whitespace().any(|campo| campo == "modo=FINITO")
            && campo_numerico(linha, "duracao")
                .is_some_and(|duracao| (1..=atraso).contains(&duracao))
            && campo_numerico(linha, "inicio").is_some_and(|inicio| {
                tique_após_prazo_rz022(tique_deadline, inicio, 0)
                    && tique_após_prazo_rz022(inicio, tique_tentativa, 0)
            })
    });
    conferir(
        finito,
        "Supervisor não registrou espera FINITO correlacionada entre deadline e tentativa",
    )?;
    Ok(())
}

fn tique_após_prazo_rz022(inicio: u64, agora: u64, atraso: u64) -> bool {
    let decorrido = agora.wrapping_sub(inicio);
    decorrido >= atraso && decorrido < (1u64 << 63)
}

fn executar_caso_recuperacao(
    raiz: &Path,
    cenario: CenarioRecuperacao,
    evidencias: &mut EvidenciasSmoke,
) -> Resultado<()> {
    let prefixo = match cenario {
        CenarioRecuperacao::CicloCompleto => "recuperacao",
        CenarioRecuperacao::RedeEmLoop => "recuperacao-loop",
    };
    let pasta = raiz.join("evidencias");
    let arquivo_serial = File::create(pasta.join(format!("serial-{prefixo}.log")))?;
    let arquivo_erros = File::create(pasta.join(format!("qemu-{prefixo}-stderr.log")))?;
    let reserva_porta = UdpSocket::bind("127.0.0.1:17777").map_err(|e| {
        format!("UDP 127.0.0.1:17777 indisponível; encerre a VM interativa antes do teste: {e}")
    })?;
    let mut comando = preparar_qemu(raiz, false, true)?;
    evidencias
        .etapas
        .push("Imagem GPT/FAT32 e bytes de BOOTX64.EFI verificados.".into());
    comando
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    drop(reserva_porta);
    let mut processo = ProcessoQemu(comando.spawn()?);
    let mut entrada = processo
        .0
        .stdin
        .take()
        .ok_or("stdin do emulador indisponível")?;
    let saida = processo
        .0
        .stdout
        .take()
        .ok_or("stdout do emulador indisponível")?;
    let erros = processo
        .0
        .stderr
        .take()
        .ok_or("stderr do emulador indisponível")?;
    let (emissor, receptor) = mpsc::channel();
    let leitores = [
        leitor_saida(saida, true, emissor.clone()),
        leitor_saida(erros, false, emissor),
    ];
    let mut captura = Captura {
        eventos: receptor,
        serial: Vec::new(),
        erros: Vec::new(),
        arquivo_serial,
        arquivo_erros,
    };
    let resultado = (|| -> Resultado<()> {
        captura.aguardar(
            &mut processo,
            "[teste] TERMINAL_PRONTO",
            0,
            Duration::from_secs(30),
        )?;
        aguardar_evento(
            &mut captura,
            &mut processo,
            0,
            "[supervisor] SERVICO_PRONTO",
            &["CPL=3", "modo=manual", "limite=4"],
        )?;
        aguardar_rede_geracao(&mut captura, &mut processo, 0, 1)?;
        conferir(
            captura
                .serial
                .windows(b"EXIT_BOOT_SERVICES_OK".len())
                .any(|w| w == b"EXIT_BOOT_SERVICES_OK"),
            "ExitBootServices não observado",
        )?;
        evidencias.etapas.push("Boot: terminal, supervisor em CPL=3/manual/limite=4 e rede em CPL=3/geração 1 prontos.".into());
        conferir_instancia(&mut entrada, &mut captura, &mut processo, 1, 1)?;
        match cenario {
            CenarioRecuperacao::CicloCompleto => {
                roteiro_recuperacao_completa(&mut entrada, &mut captura, &mut processo, evidencias)
            }
            CenarioRecuperacao::RedeEmLoop => {
                roteiro_recuperacao_loop(&mut entrada, &mut captura, &mut processo, evidencias)
            }
        }?;
        captura.drenar()?;
        let trocas = match cenario {
            CenarioRecuperacao::CicloCompleto => 3,
            CenarioRecuperacao::RedeEmLoop => 1,
        };
        let arenas = conferir_arenas_do_log(&captura.serial, trocas)?;
        let enderecos: Vec<String> = arenas.iter().map(|arena| format!("{arena:#x}")).collect();
        evidencias.etapas.push(format!("Log do kernel conferido: {trocas} trocas, {} arenas físicas distintas, encadeamento de arenas/referências preservado: {}.", arenas.len(), enderecos.join(" → ")));
        evidencias.etapas.push("Exatamente um EXIT_BOOT_SERVICES_OK observado neste cenário; não houve novo boot no log capturado.".into());
        Ok(())
    })();
    drop(entrada);
    processo.encerrar();
    let mut erro_finalizacao = None;
    for leitor in leitores {
        if leitor.join().is_err() {
            erro_finalizacao = Some("Falha na thread de captura do emulador".into());
        }
    }
    if let Err(erro) = captura.drenar() {
        erro_finalizacao = Some(erro);
    }
    captura.arquivo_serial.flush()?;
    captura.arquivo_erros.flush()?;
    match resultado {
        Err(erro) => Err(erro),
        Ok(()) => match erro_finalizacao {
            Some(erro) => Err(erro),
            None => Ok(()),
        },
    }
}

fn conferir_arenas_do_log(serial: &[u8], trocas_esperadas: usize) -> Resultado<Vec<u64>> {
    let marcador_boot = b"EXIT_BOOT_SERVICES_OK";
    let boots = serial
        .windows(marcador_boot.len())
        .filter(|trecho| *trecho == marcador_boot)
        .count();
    if boots != 1 {
        return Err(format!(
            "Esperado um único boot no cenário; observados {boots} marcadores ExitBootServices"
        )
        .into());
    }
    let mut arenas = Vec::new();
    let mut referencia_atual = None;
    let mut trocas = 0;
    for linha in String::from_utf8_lossy(serial).split_inclusive('\n') {
        if !linha.ends_with('\n') {
            continue;
        }
        if linha.contains("[microkernel] SERVICO_CARREGADO")
            && campo_numerico(linha, "id") == Some(2)
        {
            conferir(
                arenas.is_empty(),
                "Mais de um carregamento inicial da rede no mesmo cenário",
            )?;
            let arena = campo_numerico(linha, "fisico")
                .ok_or("Arena inicial da rede ausente/inválida no log")?;
            let referencia = campo_numerico(linha, "referencia")
                .ok_or("Referência inicial da rede ausente/inválida no log")?;
            conferir(
                arena != 0 && referencia == 0x1_0000_0002,
                "Arena ou referência inicial da rede inválida",
            )?;
            arenas.push(arena);
            referencia_atual = Some(referencia);
        } else if linha.contains("[microkernel] SERVICO_REINICIADO")
            && campo_numerico(linha, "id") == Some(2)
        {
            let anterior = *arenas
                .last()
                .ok_or("Reinício registrado antes do carregamento inicial da rede")?;
            let arena_antiga = campo_numerico(linha, "arena_antiga")
                .ok_or("arena_antiga ausente/inválida no log")?;
            let arena_nova =
                campo_numerico(linha, "arena_nova").ok_or("arena_nova ausente/inválida no log")?;
            let antiga = campo_numerico(linha, "antiga")
                .ok_or("Referência antiga ausente/inválida no log")?;
            let nova =
                campo_numerico(linha, "nova").ok_or("Referência nova ausente/inválida no log")?;
            let _purgadas = campo_numerico(linha, "purgadas")
                .ok_or("Contagem de purga ausente/inválida no log")?;
            if arena_antiga != anterior {
                return Err(format!("Cadeia de arenas interrompida: arena_antiga={arena_antiga:#x}, última arena={anterior:#x}").into());
            }
            if arena_nova == 0 || arenas.contains(&arena_nova) {
                return Err(format!("Arena nova inválida ou reutilizada: {arena_nova:#x}").into());
            }
            conferir(
                Some(antiga) == referencia_atual
                    && nova & 0xffff_ffff == 2
                    && (nova >> 32) == (antiga >> 32) + 1,
                "Cadeia de referências/gerações diverge no log de reinício",
            )?;
            arenas.push(arena_nova);
            referencia_atual = Some(nova);
            trocas += 1;
        }
    }
    conferir(
        !arenas.is_empty(),
        "Carregamento inicial da rede não encontrado no log",
    )?;
    if trocas != trocas_esperadas {
        return Err(
            format!("Esperadas {trocas_esperadas} trocas de arena; observadas {trocas}").into(),
        );
    }
    Ok(arenas)
}

fn campo_numerico(linha: &str, nome: &str) -> Option<u64> {
    let prefixo = format!("{nome}=");
    let mut resultado = None;
    for campo in linha.split_whitespace() {
        if let Some(valor) = campo.strip_prefix(&prefixo) {
            if resultado.is_some() {
                return None;
            }
            resultado = Some(match valor.strip_prefix("0x") {
                Some(hex) => u64::from_str_radix(hex, 16).ok()?,
                None => valor.parse().ok()?,
            });
        }
    }
    resultado
}

fn validar_log_prazo_empate(bytes: &[u8]) -> Resultado<()> {
    const PREFIXO: &str = "[teste-prazo-empate] ";
    const TERMINAL: u64 = 0x100000001;
    const REDE: u64 = 0x100000002;
    const REDE_ID: u64 = 2;
    let texto = std::str::from_utf8(bytes)?;
    let mut features = Vec::new();
    let mut armadas = Vec::new();
    let mut chamadas_receber = Vec::new();
    let mut sondas_chamadas = Vec::new();
    let mut sondas_registradas = Vec::new();
    let mut esperas = Vec::new();
    let mut timeouts = Vec::new();
    let mut admissoes = Vec::new();
    let mut resultados = Vec::new();
    for (indice, completa) in texto.split_inclusive('\n').enumerate() {
        let contem = completa.contains(PREFIXO);
        if contem && !completa.ends_with('\n') {
            return Err("Marcador T03-05 incompleto".into());
        }
        let linha = completa.trim_end_matches(['\r', '\n']);
        if linha.contains(PREFIXO) && !linha.starts_with(PREFIXO) {
            return Err("Eco ou texto antes do marcador T03-05".into());
        }
        if linha.starts_with("[microkernel] FALHA_NUCLEO")
            || linha.starts_with("[microkernel] SERVICO_FALHOU")
            || linha.starts_with("[microkernel] SERVICO_FINALIZADO")
            || linha.starts_with("[microkernel] PANICO")
            || linha.to_ascii_lowercase().contains("panic")
        {
            return Err(format!("Falha geral observada durante T03-05: {linha}").into());
        }
        if linha.starts_with("[teste-prazo-empate] FALHA")
            || linha.starts_with("[teste-prazo-empate] REGISTRO_RECUSADO")
        {
            return Err(format!("Falha declarada pela fixture: {linha}").into());
        }
        if linha == "[teste-prazo-empate] KERNEL_FEATURE=1" {
            features.push(indice);
        } else if linha.starts_with("[teste-prazo-empate] ARMADA ") {
            let campos: Vec<&str> = linha.split_whitespace().collect();
            if campos.len() != 5 || campos[1] != "ARMADA" {
                return Err("Campos extras/ausentes no marcador ARMADA".into());
            }
            armadas.push((
                indice,
                campo_numerico(linha, "origem"),
                campo_numerico(linha, "destino"),
                campo_numerico(linha, "token"),
            ));
        } else if linha.starts_with("[teste-prazo-empate] RECEBER_CHAMADA ") {
            let campos: Vec<&str> = linha.split_whitespace().collect();
            if campos.len() != 6 || campos[1] != "RECEBER_CHAMADA" {
                return Err("Campos extras/ausentes no marcador RECEBER_CHAMADA".into());
            }
            chamadas_receber.push((
                indice,
                campo_numerico(linha, "modo"),
                campo_numerico(linha, "duracao"),
                campo_numerico(linha, "tick"),
                campo_numerico(linha, "fila"),
            ));
        } else if linha.starts_with("[teste-prazo-empate] SONDA_CHAMADA ") {
            let campos: Vec<&str> = linha.split_whitespace().collect();
            if campos.len() != 7 || campos[1] != "SONDA_CHAMADA" {
                return Err("Campos extras/ausentes no marcador SONDA_CHAMADA".into());
            }
            sondas_chamadas.push((
                indice,
                campo_numerico(linha, "chamador"),
                campo_numerico(linha, "cpl"),
                campo_numerico(linha, "alvo"),
                campo_numerico(linha, "armada"),
                campo_numerico(linha, "registrada"),
            ));
        } else if linha.starts_with("[teste-prazo-empate] SONDA_REGISTRADA ") {
            let campos: Vec<&str> = linha.split_whitespace().collect();
            if campos.len() != 5 || campos[1] != "SONDA_REGISTRADA" {
                return Err("Campos extras/ausentes no marcador SONDA_REGISTRADA".into());
            }
            sondas_registradas.push((
                indice,
                campo_numerico(linha, "referencia"),
                campo_numerico(linha, "prazo"),
                campo_numerico(linha, "timeout"),
            ));
        } else if linha.starts_with("[teste-prazo-empate] ESPERA ") {
            let campos: Vec<&str> = linha.split_whitespace().collect();
            if campos.len() != 5 || campos[1] != "ESPERA" {
                return Err("Campos extras/ausentes no marcador ESPERA".into());
            }
            esperas.push((
                indice,
                campo_numerico(linha, "referencia"),
                campo_numerico(linha, "tick"),
                campo_numerico(linha, "prazo"),
            ));
        } else if linha.starts_with("[teste-prazo-empate] TIMEOUT ") {
            let campos: Vec<&str> = linha.split_whitespace().collect();
            if campos.len() != 5 || campos[1] != "TIMEOUT" {
                return Err("Campos extras/ausentes no marcador TIMEOUT".into());
            }
            timeouts.push((
                indice,
                campo_numerico(linha, "referencia"),
                campo_numerico(linha, "tick"),
                campo_numerico(linha, "prazo"),
            ));
        } else if linha.starts_with("[teste-prazo-empate] ADMISSAO ") {
            let campos: Vec<&str> = linha.split_whitespace().collect();
            if campos.len() != 8 || campos[1] != "ADMISSAO" {
                return Err("Campos extras/ausentes no marcador ADMISSAO".into());
            }
            admissoes.push((
                indice,
                campo_numerico(linha, "origem"),
                campo_numerico(linha, "destino"),
                campo_numerico(linha, "codigo"),
                campo_numerico(linha, "token"),
                campo_numerico(linha, "tick"),
                campo_numerico(linha, "prazo"),
            ));
        } else if linha.starts_with("[teste-prazo-empate] RESULTADO ") {
            let campos: Vec<&str> = linha.split_whitespace().collect();
            if campos.len() != 9
                || campos[1] != "RESULTADO"
                || campos[2] != "primeiro=TEMPO"
                || campos[3] != "segundo=IPC"
                || campos[8] != "CPL3"
            {
                return Err("Campos extras/ausentes no marcador RESULTADO".into());
            }
            resultados.push((
                indice,
                linha
                    .split_whitespace()
                    .any(|campo| campo == "primeiro=TEMPO"),
                linha.split_whitespace().any(|campo| campo == "segundo=IPC"),
                campo_numerico(linha, "origem"),
                campo_numerico(linha, "codigo"),
                campo_numerico(linha, "prazo"),
                campo_numerico(linha, "token"),
                linha.split_whitespace().any(|campo| campo == "CPL3"),
            ));
        }
    }
    if features.len() != 1 {
        return Err("Feature T03-05 ausente/duplicada no kernel opt-in".into());
    }
    if armadas.len() != 1 {
        return Err("IPC de preparação não foi admitida uma única vez".into());
    }
    if chamadas_receber.len() != 1 {
        return Err("Chamada de recepção da fixture ausente/duplicada".into());
    }
    if sondas_chamadas.len() != 1 || sondas_registradas.len() != 1 {
        return Err("Sonda da rede ausente/duplicada ou sem registro visível".into());
    }
    if esperas.len() != 1 || timeouts.len() != 1 || admissoes.len() != 1 || resultados.len() != 1 {
        return Err("Marcadores essenciais ausentes ou duplicados em T03-05".into());
    }
    let pos_feature = features[0];
    let (pos_armada, origem_armada, destino_armado, token_armado) = armadas[0];
    let (pos_chamada, modo, duracao, tick_chamada, fila) = chamadas_receber[0];
    let (pos_sonda_chamada, chamador, cpl, alvo, armada_na_sonda, registrada_na_sonda) =
        sondas_chamadas[0];
    let (pos_sonda_registrada, ref_sonda, prazo_sonda, timeout_sonda) = sondas_registradas[0];
    let (pos_espera, ref_espera, inicio, prazo) = esperas[0];
    let (pos_timeout, ref_timeout, tick_timeout, prazo_timeout) = timeouts[0];
    let (pos_admissao, origem, destino, codigo, token, tick_admissao, prazo_admissao) =
        admissoes[0];
    let (
        pos_resultado,
        primeiro_tempo,
        segundo_ipc,
        origem_resultado,
        codigo_resultado,
        prazo_resultado,
        token_resultado,
        cpl3,
    ) = resultados[0];
    let inicio = inicio.ok_or("Tick de registro ausente/inválido")?;
    let prazo = prazo.ok_or("Prazo registrado ausente/inválido")?;
    let sonda_snapshot_valido = (armada_na_sonda == Some(1) && registrada_na_sonda == Some(0))
        || (armada_na_sonda == Some(0) && registrada_na_sonda == Some(1));
    if pos_feature >= pos_armada
        || pos_armada >= pos_chamada
        || pos_chamada >= pos_espera
        || pos_armada >= pos_sonda_chamada
        || pos_sonda_chamada >= pos_sonda_registrada
        || pos_espera >= pos_sonda_registrada
        || pos_sonda_registrada >= pos_timeout
        || origem_armada != Some(TERMINAL)
        || destino_armado != Some(REDE)
        || token_armado != Some(0x0505)
        || modo != Some(1)
        || duracao != Some(40)
        || tick_chamada != Some(inicio)
        || fila != Some(0)
        || chamador != Some(REDE_ID)
        || cpl != Some(3)
        || alvo != Some(TERMINAL)
        || !sonda_snapshot_valido
        || ref_sonda != Some(TERMINAL)
        || prazo_sonda != Some(prazo)
        || !matches!(timeout_sonda, Some(0 | 1))
    {
        return Err("Sequência de feature, armação, registro e sonda diverge".into());
    }
    if ref_espera != Some(TERMINAL) || prazo != inicio.wrapping_add(40) {
        return Err("Registro não identifica a primeira espera finita do terminal".into());
    }
    if pos_espera >= pos_timeout
        || pos_timeout >= pos_admissao
        || pos_admissao >= pos_resultado
        || ref_timeout != Some(TERMINAL)
        || tick_timeout != Some(prazo)
        || prazo_timeout != Some(prazo)
    {
        return Err("Timeout e admissão não seguem a espera no mesmo tick lógico".into());
    }
    if origem != Some(REDE)
        || destino != Some(TERMINAL)
        || codigo != Some(0x7f1)
        || token != Some(0x0505)
        || tick_admissao != Some(prazo)
        || prazo_admissao != Some(prazo)
    {
        return Err("Admissão não corresponde ao envio autorizado no deadline".into());
    }
    if !primeiro_tempo
        || !segundo_ipc
        || origem_resultado != Some(REDE)
        || codigo_resultado != Some(0x7f1)
        || prazo_resultado != Some(prazo)
        || token_resultado != Some(0x0505)
        || !cpl3
    {
        return Err("Retorno único TEMPO e recepção posterior não conferem".into());
    }
    Ok(())
}

fn roteiro_recuperacao_completa(
    entrada: &mut std::process::ChildStdin,
    captura: &mut Captura,
    processo: &mut ProcessoQemu,
    evidencias: &mut EvidenciasSmoke,
) -> Resultado<()> {
    conferir_conectividade(entrada, captura, processo, 1, "inicial", evidencias)?;

    let inicio = enviar_comando(entrada, captura, "guardar-referencia\n")?;
    aguardar_evento(
        captura,
        processo,
        inicio,
        "[teste] REFERENCIA_GUARDADA",
        &["geracao=1"],
    )?;
    evidencias
        .etapas
        .push("Referência da geração 1 guardada antes da falha.".into());
    let inicio = enviar_comando(entrada, captura, "falhar-rede\n")?;
    aguardar_evento(
        captura,
        processo,
        inicio,
        "[microkernel] SERVICO_FALHOU",
        &["id=2", "vetor=6"],
    )?;
    conferir_eco_terminal(entrada, captura, processo, "TERMINAL_VIVO_ANTES_REINICIO")?;
    conferir_instancia(entrada, captura, processo, 2, 1)?;
    evidencias.etapas.push("UD2 encerrou a execução da rede na geração 1; terminal respondeu e observou rede=2 antes de solicitar reinício manual.".into());
    let inicio = enviar_comando(entrada, captura, "reiniciar-rede\n")?;
    aguardar_reinicio(captura, processo, inicio, 1, 2, false)?;
    conferir_instancia(entrada, captura, processo, 1, 2)?;
    evidencias.etapas.push("Supervisor confirmou reinício manual da geração 1 para a 2; nova rede pronta e executável.".into());

    let inicio = enviar_comando(entrada, captura, "teste-referencia\n")?;
    aguardar_evento(
        captura,
        processo,
        inicio,
        "[teste] REFERENCIA_ANTIGA_ENVIO_OK",
        &[],
    )?;
    aguardar_evento(
        captura,
        processo,
        inicio,
        "[teste] REFERENCIA_ANTIGA_ESTADO_OK",
        &[],
    )?;
    evidencias.etapas.push("Envio IPC e consulta de estado com referência da geração 1 foram recusados após a geração 2 iniciar.".into());
    let inicio = enviar_comando(entrada, captura, "teste-supervisor\n")?;
    aguardar_evento(
        captura,
        processo,
        inicio,
        "[teste] SUPERVISOR_REINICIAR_NEGADO_OK",
        &[],
    )?;
    aguardar_evento(
        captura,
        processo,
        inicio,
        "[teste] SUPERVISOR_ENCERRAR_NEGADO_OK",
        &[],
    )?;
    conferir_instancia(entrada, captura, processo, 1, 2)?;
    evidencias.etapas.push("Chamadas diretas REINICIAR e ENCERRAR do terminal foram negadas; rede permaneceu ativa na geração 2.".into());
    conferir_conectividade(
        entrada,
        captura,
        processo,
        2,
        "após reinício manual e negativas de autoridade",
        evidencias,
    )?;

    let inicio = enviar_comando(entrada, captura, "supervisao auto\n")?;
    aguardar_evento(
        captura,
        processo,
        inicio,
        "[supervisor] MODO auto",
        &["limite=4"],
    )?;
    let inicio = enviar_comando(entrada, captura, "violar-memoria\n")?;
    aguardar_evento(
        captura,
        processo,
        inicio,
        "[microkernel] SERVICO_FALHOU",
        &["id=2", "vetor=14", "CR2=0x2000000"],
    )?;
    aguardar_reinicio(captura, processo, inicio, 2, 3, true)?;
    conferir_eco_terminal(entrada, captura, processo, "RECUPERACAO_AUTOMATICA_OK")?;
    conferir_instancia(entrada, captura, processo, 1, 3)?;
    evidencias.etapas.push("Em modo automático, page fault da geração 2 foi seguido de reinício confirmado pelo supervisor para a geração 3, com terminal vivo.".into());
    conferir_conectividade(
        entrada,
        captura,
        processo,
        3,
        "após recuperação automática",
        evidencias,
    )?;

    let inicio = enviar_comando(entrada, captura, "parar-rede\n")?;
    aguardar_evento(
        captura,
        processo,
        inicio,
        "[supervisor] REDE_PARADA",
        &["geracao=3"],
    )?;
    let consulta = enviar_comando(entrada, captura, "supervisao\n")?;
    aguardar_evento(
        captura,
        processo,
        consulta,
        "[supervisor] MODO manual",
        &["limite=4"],
    )?;
    conferir_instancia(entrada, captura, processo, 3, 3)?;
    conferir_eco_terminal(entrada, captura, processo, "TERMINAL_VIVO_REDE_PARADA")?;
    evidencias.etapas.push("parar-rede mudou a política para manual e deixou a geração 3 em estado finalizado (rede=3), com terminal responsivo.".into());
    let inicio = enviar_comando(entrada, captura, "reiniciar-rede\n")?;
    aguardar_reinicio(captura, processo, inicio, 3, 4, false)?;
    conferir_instancia(entrada, captura, processo, 1, 4)?;
    evidencias.etapas.push(
        "Supervisor retomou a rede por reinício manual da geração 3 para a geração 4.".into(),
    );
    conferir_conectividade(
        entrada,
        captura,
        processo,
        4,
        "após parada e retomada",
        evidencias,
    )?;

    let inicio = enviar_comando(entrada, captura, "reiniciar-rede\n")?;
    aguardar_evento(
        captura,
        processo,
        inicio,
        "[supervisor] RECURSOS_ESGOTADOS",
        &["operacao=9", "geracao=4"],
    )?;
    conferir_instancia(entrada, captura, processo, 1, 4)?;
    conferir_conectividade(
        entrada,
        captura,
        processo,
        4,
        "após recusa da quinta encarnação",
        evidencias,
    )?;
    conferir_eco_terminal(entrada, captura, processo, "QUOTA_REDE_PRESERVADA_OK")?;
    evidencias.etapas.push("Quinta encarnação recusada por RECURSOS; a geração 4 permaneceu ativa, respondeu a ping/UDP e o terminal continuou responsivo.".into());
    Ok(())
}

fn roteiro_recuperacao_loop(
    entrada: &mut std::process::ChildStdin,
    captura: &mut Captura,
    processo: &mut ProcessoQemu,
    evidencias: &mut EvidenciasSmoke,
) -> Resultado<()> {
    let inicio = enviar_comando(entrada, captura, "ocupar-rede\n")?;
    aguardar_evento(captura, processo, inicio, "[rede] LOOP_SEM_CEDER", &[])?;
    thread::sleep(Duration::from_millis(200));
    conferir_eco_terminal(entrada, captura, processo, "TERMINAL_VIVO_REDE_EM_LOOP")?;
    conferir_instancia(entrada, captura, processo, 1, 1)?;
    evidencias.etapas.push("Rede na geração 1 confirmou loop sem ceder; terminal continuou executando antes do reinício.".into());
    let inicio = enviar_comando(entrada, captura, "reiniciar-rede\n")?;
    aguardar_reinicio(captura, processo, inicio, 1, 2, false)?;
    conferir_instancia(entrada, captura, processo, 1, 2)?;
    conferir_conectividade(
        entrada,
        captura,
        processo,
        2,
        "após reinício da rede presa em loop",
        evidencias,
    )?;
    conferir_eco_terminal(entrada, captura, processo, "REDE_RECUPERADA_DO_LOOP_OK")?;
    evidencias.etapas.push("Supervisor reiniciou o serviço ativo preso em loop para a geração 2; terminal e rede voltaram a responder às verificações.".into());
    Ok(())
}

// A UART emulada possui FIFO limitada. O PIT opera a 100 Hz e há três
// serviços; 50 ms entre bytes dá oportunidades de escalonamento ao terminal
// mesmo quando a rede não cede. O oráculo ainda exige a resposta completa.
fn enviar_serial(entrada: &mut std::process::ChildStdin, bytes: &[u8]) -> Resultado<()> {
    for byte in bytes {
        entrada.write_all(&[*byte])?;
        entrada.flush()?;
        thread::sleep(Duration::from_millis(50));
    }
    Ok(())
}

fn enviar_comando(
    entrada: &mut std::process::ChildStdin,
    captura: &mut Captura,
    comando: &str,
) -> Resultado<usize> {
    captura.drenar()?;
    let inicio = captura.serial.len();
    enviar_serial(entrada, comando.as_bytes())?;
    Ok(inicio)
}

fn aguardar_evento(
    captura: &mut Captura,
    processo: &mut ProcessoQemu,
    desde: usize,
    marcador: &str,
    campos: &[&str],
) -> Resultado<()> {
    let descricao = format!("{marcador} {} (linha completa)", campos.join(" "));
    captura.aguardar_condicao(
        processo,
        &descricao,
        desde,
        Duration::from_secs(10),
        |bytes| evento_completo_presente(bytes, marcador, campos),
    )
}

fn evento_completo_presente(bytes: &[u8], marcador: &str, campos: &[&str]) -> bool {
    String::from_utf8_lossy(bytes)
        .split_inclusive('\n')
        .any(|linha| evento_em_linha_completa(linha, marcador, campos))
}

fn evento_em_linha_completa(linha: &str, marcador: &str, campos: &[&str]) -> bool {
    linha.ends_with('\n')
        && linha.trim_start().starts_with(marcador)
        && campos
            .iter()
            .all(|esperado| linha.split_whitespace().any(|campo| campo == *esperado))
}

fn referencia_rede_geracao(geracao: u64) -> u64 {
    (geracao << 32) | 2
}

fn reinicio_kernel_corresponde(linha: &str, antiga: u64, nova: u64) -> bool {
    if !linha.ends_with('\n') {
        return false;
    }
    let linha = linha.trim_end_matches(['\r', '\n']);
    let linha = linha.strip_prefix("raiz> ").unwrap_or(linha);
    let antiga_esperada = format!("antiga={antiga:#x}");
    let nova_esperada = format!("nova={nova:#x}");
    let mut campos = linha.split_whitespace();
    if campos.next() != Some("[microkernel]")
        || campos.next() != Some("SERVICO_REINICIADO")
        || campos.next() != Some("id=2")
        || campos.next() != Some(antiga_esperada.as_str())
        || campos.next() != Some(nova_esperada.as_str())
    {
        return false;
    }
    let Some(arena_antiga) = campos
        .next()
        .and_then(|campo| campo.strip_prefix("arena_antiga=0x"))
        .and_then(|valor| u64::from_str_radix(valor, 16).ok())
    else {
        return false;
    };
    let Some(arena_nova) = campos
        .next()
        .and_then(|campo| campo.strip_prefix("arena_nova=0x"))
        .and_then(|valor| u64::from_str_radix(valor, 16).ok())
    else {
        return false;
    };
    let Some(_purgadas) = campos
        .next()
        .and_then(|campo| campo.strip_prefix("purgadas="))
        .and_then(|valor| valor.parse::<u64>().ok())
    else {
        return false;
    };
    arena_antiga != 0 && arena_nova != 0 && arena_antiga != arena_nova && campos.next().is_none()
}

fn aguardar_reinicio_kernel(
    captura: &mut Captura,
    processo: &mut ProcessoQemu,
    desde: usize,
    antiga: u64,
    nova: u64,
) -> Resultado<()> {
    let marcador = format!(
        "[microkernel] SERVICO_REINICIADO antiga={antiga:#x} nova={nova:#x} (linha completa)"
    );
    captura.aguardar_condicao(
        processo,
        &marcador,
        desde,
        Duration::from_secs(10),
        |bytes| {
            String::from_utf8_lossy(bytes)
                .split_inclusive('\n')
                .any(|linha| reinicio_kernel_corresponde(linha, antiga, nova))
        },
    )
}

fn aguardar_rede_geracao(
    captura: &mut Captura,
    processo: &mut ProcessoQemu,
    desde: usize,
    geracao: u64,
) -> Resultado<()> {
    let campo = format!("geracao={geracao}");
    aguardar_evento(
        captura,
        processo,
        desde,
        "[rede] SERVICO_PRONTO",
        &["CPL=3", &campo],
    )
}

fn posicao_rede_pronta_ciclo(serial: &[u8], geracao: u64) -> Option<usize> {
    let mut resultado = None;
    let mut deslocamento = 0usize;
    for registro in serial.split_inclusive(|b| *b == b'\n') {
        if registro.ends_with(b"\n") {
            let linha = String::from_utf8_lossy(registro);
            let linha = linha.trim_end_matches(['\r', '\n']);
            let linha = linha.strip_prefix("raiz> ").unwrap_or(linha);
            if linha.starts_with("[rede] SERVICO_PRONTO") {
                let campos: Vec<_> = linha.split_ascii_whitespace().collect();
                if campos.len() != 6
                    || campos[0] != "[rede]"
                    || campos[1] != "SERVICO_PRONTO"
                    || campos[2] != "CPL=3"
                    || campos[3] != "IPv4=10.0.2.15"
                    || campos[4] != "UDP=7777"
                {
                    return None;
                }
                let geracao_linha = campos[5].strip_prefix("geracao=")?.parse::<u64>().ok()?;
                if geracao_linha == 0 {
                    return None;
                }
                if geracao_linha == geracao {
                    if resultado.replace(deslocamento).is_some() {
                        return None;
                    }
                }
            }
        }
        deslocamento += registro.len();
    }
    resultado
}

fn validar_pronto_antes_driver_ciclo(serial: &[u8], referencia: u64) -> Resultado<()> {
    let geracao = referencia >> 32;
    let pronto = posicao_rede_pronta_ciclo(serial, geracao)
        .ok_or("SERVICO_PRONTO único da geração ausente ou malformado")?;
    let prefixo = format!("[teste-ciclo-espera] DRIVER_ATIVO referencia={referencia:#x}");
    let driver =
        posicao_marcador_ciclo(serial, &prefixo).ok_or("DRIVER_ATIVO único da geração ausente")?;
    if pronto >= driver {
        return Err("SERVICO_PRONTO não precede DRIVER_ATIVO da mesma geração".into());
    }
    Ok(())
}

fn aguardar_reinicio(
    captura: &mut Captura,
    processo: &mut ProcessoQemu,
    desde: usize,
    antiga: u64,
    nova: u64,
    automatico: bool,
) -> Resultado<()> {
    let campo_antiga = format!("geracao_antiga={antiga}");
    let campo_nova = format!("geracao_nova={nova}");
    let campo_modo = format!("automatico={}", u8::from(automatico));
    aguardar_evento(
        captura,
        processo,
        desde,
        "[supervisor] REDE_REINICIADA",
        &[&campo_antiga, &campo_nova, &campo_modo],
    )?;
    aguardar_rede_geracao(captura, processo, desde, nova)
}

fn conferir_instancia(
    entrada: &mut std::process::ChildStdin,
    captura: &mut Captura,
    processo: &mut ProcessoQemu,
    estado_rede: u64,
    geracao: u64,
) -> Resultado<()> {
    let inicio = enviar_comando(entrada, captura, "processos\n")?;
    captura.aguardar_condicao(
        processo,
        "estados dos processos",
        inicio,
        Duration::from_secs(10),
        |bytes| ultimo_estado_processos(bytes).is_some(),
    )?;
    let campo = format!("geracao_rede={geracao}");
    aguardar_evento(
        captura,
        processo,
        inicio,
        "supervisor=",
        &["supervisor=1", &campo],
    )?;
    let (terminal, rede, tiques) =
        ultimo_estado_processos(&captura.serial[inicio..]).ok_or("Resposta de estados ausente")?;
    if terminal != 1 || rede != estado_rede {
        return Err(format!("Esperado terminal=1 rede={estado_rede} geração={geracao}; observado terminal={terminal} rede={rede} tiques={tiques}").into());
    }
    Ok(())
}

fn conferir_eco_terminal(
    entrada: &mut std::process::ChildStdin,
    captura: &mut Captura,
    processo: &mut ProcessoQemu,
    texto: &str,
) -> Resultado<()> {
    let inicio = enviar_comando(entrada, captura, &format!("eco {texto}\n"))?;
    captura.aguardar_linha(processo, texto, inicio, Duration::from_secs(10))
}

fn conferir_conectividade(
    entrada: &mut std::process::ChildStdin,
    captura: &mut Captura,
    processo: &mut ProcessoQemu,
    geracao: u64,
    contexto: &str,
    evidencias: &mut EvidenciasSmoke,
) -> Resultado<()> {
    let inicio = enviar_comando(entrada, captura, "ping 10.0.2.2\n")?;
    aguardar_evento(captura, processo, inicio, "[rede] PING OK 10.0.2.2", &[])?;
    let socket = socket_eco()?;
    let rodada = evidencias.pacotes_udp;
    for tamanho in [0usize, 1, 17, 1472] {
        let dados: Vec<u8> = (0..tamanho)
            .map(|i| ((i * 31 + tamanho + geracao as usize * 43 + rodada) % 251) as u8)
            .collect();
        conferir_eco(&socket, &dados)?;
        evidencias.pacotes_udp += 1;
        evidencias.bytes_udp += dados.len();
    }
    evidencias.etapas.push(format!("Geração {geracao}, {contexto}: ping do gateway recebido e ecos UDP de 0/1/17/1472 bytes conferidos."));
    Ok(())
}

fn socket_eco() -> Resultado<UdpSocket> {
    let socket = UdpSocket::bind("127.0.0.1:0")?;
    socket.connect("127.0.0.1:17777")?;
    socket.set_read_timeout(Some(Duration::from_secs(3)))?;
    socket.set_write_timeout(Some(Duration::from_secs(3)))?;
    Ok(socket)
}

fn conferir_eco(socket: &UdpSocket, dados: &[u8]) -> Resultado<()> {
    conferir(
        dados.len() <= 1472,
        "O eco de laboratório aceita payloads de até 1472 bytes",
    )?;
    let enviados = socket.send(dados)?;
    conferir(enviados == dados.len(), "Envio UDP incompleto")?;
    let mut recebido = [0u8; 65_536];
    let tamanho = socket.recv(&mut recebido).map_err(|erro| {
        format!(
            "Eco UDP de {} bytes sem resposta válida em até 3 segundos: {erro}",
            dados.len()
        )
    })?;
    conferir(
        &recebido[..tamanho] == dados,
        "Resposta UDP diferente do payload enviado",
    )
}

fn eco_udp(dados: &[u8]) -> Resultado<()> {
    let socket = socket_eco()?;
    conferir_eco(&socket, dados)?;
    println!(
        "Eco confirmado ({} bytes): {}",
        dados.len(),
        String::from_utf8_lossy(dados)
    );
    Ok(())
}

fn opcao_caminho(caminho: &Path) -> String {
    caminho.to_string_lossy().replace(',', ",,")
}

fn localizar_firmware() -> Resultado<PathBuf> {
    if let Some(caminho) = env::var_os("RAIZ_OVMF") {
        let caminho = PathBuf::from(caminho);
        if caminho.is_file() {
            return Ok(caminho);
        }
        return Err(format!(
            "RAIZ_OVMF não aponta para um arquivo: {}",
            caminho.display()
        )
        .into());
    }
    for candidato in [
        "/usr/share/OVMF/OVMF_CODE_4M.fd",
        "/usr/share/OVMF/OVMF_CODE.fd",
        "/usr/share/edk2/x64/OVMF_CODE.fd",
        "/usr/share/qemu/OVMF_CODE.fd",
        "/usr/share/edk2/ovmf/OVMF_CODE.fd",
    ] {
        let caminho = PathBuf::from(candidato);
        if caminho.is_file() {
            return Ok(caminho);
        }
    }
    Err("Firmware UEFI não encontrado. Defina RAIZ_OVMF com o caminho para OVMF_CODE.fd (ou OVMF_CODE_4M.fd) e RAIZ_OVMF_VARS com o arquivo VARS correspondente".into())
}

fn localizar_variaveis(firmware: &Path) -> Option<PathBuf> {
    if let Some(caminho) = env::var_os("RAIZ_OVMF_VARS") {
        return Some(PathBuf::from(caminho));
    }
    let nome = firmware.file_name()?.to_str()?.replace("CODE", "VARS");
    let candidato = firmware.with_file_name(nome);
    (candidato != firmware && candidato.is_file()).then_some(candidato)
}

#[derive(Clone, Copy)]
struct Geometria {
    setores_fat: u32,
    clusters: u32,
    inicio_dados: u64,
}

fn geometria() -> Geometria {
    // Resolve FAT >= ceil((clusters + 2) * 4 / 512), sendo SPC=1 e duas FATs.
    let setores_fat =
        ((u64::from(SETORES_ESP - RESERVADOS) + 2) * 4).div_ceil(SETOR as u64 + 8) as u32;
    let clusters = SETORES_ESP - RESERVADOS - 2 * setores_fat;
    assert!(clusters >= 65_525 && clusters < 0x0fff_fff5);
    assert!(u64::from(setores_fat) * 128 >= u64::from(clusters) + 2);
    Geometria {
        setores_fat,
        clusters,
        inicio_dados: INICIO_ESP + u64::from(RESERVADOS + 2 * setores_fat),
    }
}

fn criar_imagem(caminho: &Path, efi: &[u8]) -> Resultado<()> {
    let g = geometria();
    let clusters_arquivo = efi.len().div_ceil(SETOR);
    if efi.is_empty() || efi.len() > u32::MAX as usize || clusters_arquivo + 3 > g.clusters as usize
    {
        return Err("Carregador vazio ou maior que a capacidade da ESP".into());
    }
    if let Ok(meta) = fs::symlink_metadata(caminho) {
        if !meta.is_file() || meta.file_type().is_symlink() {
            return Err("O destino da imagem deve ser um arquivo regular".into());
        }
    }
    let temporario = caminho.with_extension(format!("img.{}.nova", std::process::id()));
    let mut arquivo = fs::OpenOptions::new()
        .write(true)
        .read(true)
        .create_new(true)
        .open(&temporario)?;
    let resultado = (|| -> Resultado<()> {
        // Arquivo novo e estendido: todos os intervalos não escritos leem como zero.
        arquivo.set_len(SETORES_DISCO * SETOR as u64)?;
        let mut mbr = [0u8; SETOR];
        mbr[447..450].copy_from_slice(&[0, 2, 0]);
        mbr[450] = 0xee;
        mbr[451..454].copy_from_slice(&[0xff, 0xff, 0xff]);
        colocar32(&mut mbr, 454, 1);
        colocar32(&mut mbr, 458, (SETORES_DISCO - 1) as u32);
        mbr[510..512].copy_from_slice(&[0x55, 0xaa]);
        gravar(&mut arquivo, 0, &mbr)?;

        let mut entradas = vec![0u8; TAM_GPT];
        entradas[0..16].copy_from_slice(&TIPO_ESP);
        entradas[16..32].copy_from_slice(&ID_ESP);
        colocar64(&mut entradas, 32, INICIO_ESP);
        colocar64(&mut entradas, 40, FIM_ESP);
        for (indice, letra) in "Raiz OS ESP".encode_utf16().enumerate() {
            colocar16(&mut entradas, 56 + indice * 2, letra);
        }
        let crc_entradas = crc32(&entradas);
        gravar(&mut arquivo, 2, &entradas)?;
        gravar(&mut arquivo, SETORES_DISCO - 33, &entradas)?;
        gravar(
            &mut arquivo,
            1,
            &cabecalho_gpt(1, SETORES_DISCO - 1, 2, crc_entradas),
        )?;
        gravar(
            &mut arquivo,
            SETORES_DISCO - 1,
            &cabecalho_gpt(SETORES_DISCO - 1, 1, SETORES_DISCO - 33, crc_entradas),
        )?;

        let boot = setor_fat32(g);
        gravar(&mut arquivo, INICIO_ESP, &boot)?;
        gravar(&mut arquivo, INICIO_ESP + 6, &boot)?;
        let mut info = [0u8; SETOR];
        colocar32(&mut info, 0, 0x4161_5252);
        colocar32(&mut info, 484, 0x6141_7272);
        let livres = g.clusters - clusters_arquivo as u32 - 3;
        colocar32(&mut info, 488, livres);
        colocar32(
            &mut info,
            492,
            if livres == 0 {
                u32::MAX
            } else {
                5 + clusters_arquivo as u32
            },
        );
        colocar32(&mut info, 508, 0xaa55_0000);
        gravar(&mut arquivo, INICIO_ESP + 1, &info)?;
        gravar(&mut arquivo, INICIO_ESP + 7, &info)?;

        let mut fat = vec![0u8; g.setores_fat as usize * SETOR];
        colocar32(&mut fat, 0, 0x0fff_fff8);
        for cluster in 1..=4 {
            colocar32(&mut fat, cluster * 4, EOF_FAT);
        }
        for indice in 0..clusters_arquivo {
            let cluster = indice + 5;
            let proximo = if indice + 1 == clusters_arquivo {
                EOF_FAT
            } else {
                cluster as u32 + 1
            };
            colocar32(&mut fat, cluster * 4, proximo);
        }
        gravar(&mut arquivo, INICIO_ESP + u64::from(RESERVADOS), &fat)?;
        gravar(
            &mut arquivo,
            INICIO_ESP + u64::from(RESERVADOS + g.setores_fat),
            &fat,
        )?;

        let mut raiz = [0u8; SETOR];
        entrada_diretorio(&mut raiz, 0, b"RAIZOS     ", 0x08, 0, 0);
        entrada_diretorio(&mut raiz, 1, b"EFI        ", 0x10, 3, 0);
        gravar(&mut arquivo, g.inicio_dados, &raiz)?;
        let mut pasta_efi = [0u8; SETOR];
        entrada_diretorio(&mut pasta_efi, 0, b".          ", 0x10, 3, 0);
        entrada_diretorio(&mut pasta_efi, 1, b"..         ", 0x10, 0, 0);
        entrada_diretorio(&mut pasta_efi, 2, b"BOOT       ", 0x10, 4, 0);
        gravar(&mut arquivo, g.inicio_dados + 1, &pasta_efi)?;
        let mut pasta_boot = [0u8; SETOR];
        entrada_diretorio(&mut pasta_boot, 0, b".          ", 0x10, 4, 0);
        entrada_diretorio(&mut pasta_boot, 1, b"..         ", 0x10, 3, 0);
        entrada_diretorio(
            &mut pasta_boot,
            2,
            b"BOOTX64 EFI",
            0x20,
            5,
            efi.len() as u32,
        );
        gravar(&mut arquivo, g.inicio_dados + 2, &pasta_boot)?;
        gravar(&mut arquivo, g.inicio_dados + 3, efi)?;
        arquivo.sync_all()?;
        Ok(())
    })();
    drop(arquivo);
    if let Err(erro) = resultado {
        let _ = fs::remove_file(&temporario);
        return Err(erro);
    }
    if cfg!(windows) && caminho.is_file() {
        fs::remove_file(caminho)?;
    }
    fs::rename(&temporario, caminho)?;
    Ok(())
}

fn cabecalho_gpt(atual: u64, outro: u64, inicio_entradas: u64, crc_entradas: u32) -> [u8; SETOR] {
    let mut b = [0u8; SETOR];
    b[0..8].copy_from_slice(b"EFI PART");
    colocar32(&mut b, 8, 0x0001_0000);
    colocar32(&mut b, 12, 92);
    colocar64(&mut b, 24, atual);
    colocar64(&mut b, 32, outro);
    colocar64(&mut b, 40, 34);
    colocar64(&mut b, 48, SETORES_DISCO - 34);
    b[56..72].copy_from_slice(&ID_DISCO);
    colocar64(&mut b, 72, inicio_entradas);
    colocar32(&mut b, 80, ENTRADAS_GPT as u32);
    colocar32(&mut b, 84, TAM_ENTRADA_GPT as u32);
    colocar32(&mut b, 88, crc_entradas);
    let crc = crc32(&b[..92]);
    colocar32(&mut b, 16, crc);
    b
}

fn setor_fat32(g: Geometria) -> [u8; SETOR] {
    let mut b = [0u8; SETOR];
    b[0..3].copy_from_slice(&[0xeb, 0x58, 0x90]);
    b[3..11].copy_from_slice(b"RAIZOS  ");
    colocar16(&mut b, 11, SETOR as u16);
    b[13] = 1; // Um setor por cluster.
    colocar16(&mut b, 14, RESERVADOS as u16);
    b[16] = 2; // Duas FATs espelhadas.
    b[21] = 0xf8;
    colocar16(&mut b, 24, 63);
    colocar16(&mut b, 26, 255);
    colocar32(&mut b, 28, INICIO_ESP as u32);
    colocar32(&mut b, 32, SETORES_ESP);
    colocar32(&mut b, 36, g.setores_fat);
    colocar32(&mut b, 44, 2); // Diretório raiz.
    colocar16(&mut b, 48, 1); // FSInfo.
    colocar16(&mut b, 50, 6); // Cópia do boot sector.
    b[64] = 0x80;
    b[66] = 0x29;
    colocar32(&mut b, 67, 0x5241_495a);
    b[71..82].copy_from_slice(b"RAIZOS     ");
    b[82..90].copy_from_slice(b"FAT32   ");
    // Não há boot BIOS. Caso executado por BIOS legado, para sem escrever disco.
    b[90..94].copy_from_slice(&[0xfa, 0xf4, 0xeb, 0xfd]);
    b[510..512].copy_from_slice(&[0x55, 0xaa]);
    b
}

fn entrada_diretorio(
    b: &mut [u8],
    indice: usize,
    nome: &[u8; 11],
    atributo: u8,
    cluster: u32,
    tamanho: u32,
) {
    let inicio = indice * 32;
    let e = &mut b[inicio..inicio + 32];
    e[..11].copy_from_slice(nome);
    e[11] = atributo;
    // Data fixa torna a imagem reproduzível a partir dos mesmos bytes EFI.
    let data = ((2026 - 1980) << 9) | (11 << 5) | 9;
    colocar16(e, 16, data);
    colocar16(e, 18, data);
    colocar16(e, 20, (cluster >> 16) as u16);
    colocar16(e, 24, data);
    colocar16(e, 26, cluster as u16);
    colocar32(e, 28, tamanho);
}

fn verificar_imagem(caminho: &Path, efi: &[u8]) -> Resultado<()> {
    let mut arquivo = File::open(caminho)?;
    conferir(
        arquivo.metadata()?.len() == SETORES_DISCO * SETOR as u64,
        "Tamanho de imagem inesperado",
    )?;
    let mbr = ler(&mut arquivo, 0, SETOR)?;
    conferir(
        mbr[510..512] == [0x55, 0xaa] && mbr[450] == 0xee,
        "MBR protetor inválido",
    )?;
    conferir(
        ler32(&mbr, 454) == 1 && ler32(&mbr, 458) == (SETORES_DISCO - 1) as u32,
        "Faixa do MBR protetor inválida",
    )?;
    let entradas = ler(&mut arquivo, 2, TAM_GPT)?;
    let reserva = ler(&mut arquivo, SETORES_DISCO - 33, TAM_GPT)?;
    conferir(entradas == reserva, "Cópias da tabela GPT divergem")?;
    let crc = crc32(&entradas);
    for (atual, outro, inicio_entradas) in [
        (1, SETORES_DISCO - 1, 2),
        (SETORES_DISCO - 1, 1, SETORES_DISCO - 33),
    ] {
        let mut h = ler(&mut arquivo, atual, SETOR)?;
        conferir(
            &h[..8] == b"EFI PART" && ler32(&h, 8) == 0x0001_0000 && ler32(&h, 12) == 92,
            "Assinatura, versão ou tamanho GPT inválidos",
        )?;
        let esperado = ler32(&h, 16);
        colocar32(&mut h, 16, 0);
        conferir(crc32(&h[..92]) == esperado, "CRC do cabeçalho GPT inválido")?;
        conferir(
            ler64(&h, 24) == atual && ler64(&h, 32) == outro && ler64(&h, 72) == inicio_entradas,
            "Endereços GPT inválidos",
        )?;
        conferir(
            ler64(&h, 40) == 34 && ler64(&h, 48) == FIM_ESP && h[56..72] == ID_DISCO,
            "Geometria/identificador GPT inválidos",
        )?;
        conferir(
            ler32(&h, 80) == ENTRADAS_GPT as u32
                && ler32(&h, 84) == TAM_ENTRADA_GPT as u32
                && ler32(&h, 88) == crc,
            "Dimensões/CRC da tabela GPT inválidos",
        )?;
    }
    conferir(
        entradas[..16] == TIPO_ESP
            && entradas[16..32] == ID_ESP
            && ler64(&entradas, 32) == INICIO_ESP
            && ler64(&entradas, 40) == FIM_ESP,
        "Partição ESP inválida",
    )?;
    let g = geometria();
    let boot = ler(&mut arquivo, INICIO_ESP, SETOR)?;
    conferir(
        boot[510..512] == [0x55, 0xaa] && ler16(&boot, 11) == SETOR as u16 && boot[13] == 1,
        "Boot sector FAT32 inválido",
    )?;
    conferir(
        ler16(&boot, 14) == RESERVADOS as u16
            && boot[16] == 2
            && ler16(&boot, 17) == 0
            && ler16(&boot, 19) == 0
            && ler16(&boot, 22) == 0
            && ler32(&boot, 28) == INICIO_ESP as u32
            && ler32(&boot, 32) == SETORES_ESP
            && ler32(&boot, 36) == g.setores_fat
            && ler32(&boot, 44) == 2
            && ler16(&boot, 48) == 1
            && ler16(&boot, 50) == 6,
        "Geometria FAT32 inválida",
    )?;
    conferir(
        ler(&mut arquivo, INICIO_ESP + 6, SETOR)? == boot,
        "Cópia do boot sector FAT32 diverge",
    )?;
    let info = ler(&mut arquivo, INICIO_ESP + 1, SETOR)?;
    conferir(
        ler32(&info, 0) == 0x4161_5252
            && ler32(&info, 484) == 0x6141_7272
            && ler32(&info, 508) == 0xaa55_0000,
        "FSInfo inválido",
    )?;
    conferir(
        ler(&mut arquivo, INICIO_ESP + 7, SETOR)? == info,
        "Cópias de FSInfo divergem",
    )?;
    let fat = ler(
        &mut arquivo,
        INICIO_ESP + u64::from(RESERVADOS),
        g.setores_fat as usize * SETOR,
    )?;
    let fat2 = ler(
        &mut arquivo,
        INICIO_ESP + u64::from(RESERVADOS + g.setores_fat),
        fat.len(),
    )?;
    conferir(fat == fat2, "Cópias da FAT divergem")?;
    conferir(
        ler32(&fat, 0) & 0x0fff_ffff == 0x0fff_fff8 && ler32(&fat, 4) & 0x0fff_ffff >= 0x0fff_fff8,
        "Entradas reservadas FAT inválidas",
    )?;
    let pasta_efi = encontrar_entrada(&mut arquivo, g, &fat, 2, b"EFI        ", 0x10)?.0;
    let pasta_boot = encontrar_entrada(&mut arquivo, g, &fat, pasta_efi, b"BOOT       ", 0x10)?.0;
    let (mut cluster, tamanho) =
        encontrar_entrada(&mut arquivo, g, &fat, pasta_boot, b"BOOTX64 EFI", 0x20)?;
    conferir(
        tamanho as usize == efi.len(),
        "Tamanho do BOOTX64.EFI diverge",
    )?;
    let mut posicao = 0;
    let mut visitados = 0;
    while posicao < efi.len() {
        conferir(
            cluster >= 2 && cluster < g.clusters + 2,
            "Cluster do arquivo fora da ESP",
        )?;
        visitados += 1;
        conferir(visitados <= g.clusters, "Ciclo na cadeia FAT")?;
        let bloco = ler(&mut arquivo, g.inicio_dados + u64::from(cluster - 2), SETOR)?;
        let quantidade = (efi.len() - posicao).min(SETOR);
        conferir(
            bloco[..quantidade] == efi[posicao..posicao + quantidade],
            "Bytes de BOOTX64.EFI divergem",
        )?;
        posicao += quantidade;
        cluster = ler32(&fat, cluster as usize * 4) & 0x0fff_ffff;
    }
    conferir(
        cluster >= 0x0fff_fff8,
        "Cadeia FAT não termina após o carregador",
    )?;
    Ok(())
}

fn encontrar_entrada(
    arquivo: &mut File,
    g: Geometria,
    fat: &[u8],
    cluster: u32,
    nome: &[u8; 11],
    atributo: u8,
) -> Resultado<(u32, u32)> {
    conferir(
        cluster >= 2 && cluster < g.clusters + 2,
        "Cluster do diretório inválido",
    )?;
    // Este formato de imagem utiliza diretórios com exatamente um cluster.
    conferir(
        ler32(fat, cluster as usize * 4) & 0x0fff_ffff >= 0x0fff_fff8,
        "Diretório excedeu um cluster",
    )?;
    let dados = ler(arquivo, g.inicio_dados + u64::from(cluster - 2), SETOR)?;
    for e in dados.chunks_exact(32) {
        if e[0] == 0 {
            break;
        }
        if &e[..11] == nome && e[11] == atributo {
            let cluster = ((ler16(e, 20) as u32) << 16) | ler16(e, 26) as u32;
            return Ok((cluster, ler32(e, 28)));
        }
    }
    Err(format!("Entrada ausente na ESP: {}", String::from_utf8_lossy(nome)).into())
}

fn conferir(condicao: bool, mensagem: &'static str) -> Resultado<()> {
    if condicao {
        Ok(())
    } else {
        Err(mensagem.into())
    }
}
fn gravar(arquivo: &mut File, setor: u64, dados: &[u8]) -> io::Result<()> {
    arquivo.seek(SeekFrom::Start(setor * SETOR as u64))?;
    arquivo.write_all(dados)
}
fn ler(arquivo: &mut File, setor: u64, tamanho: usize) -> io::Result<Vec<u8>> {
    let inicio = setor
        .checked_mul(SETOR as u64)
        .ok_or_else(|| io::Error::other("Offset inválido"))?;
    let fim = inicio
        .checked_add(tamanho as u64)
        .ok_or_else(|| io::Error::other("Tamanho inválido"))?;
    if fim > SETORES_DISCO * SETOR as u64 {
        return Err(io::Error::other("Leitura fora da imagem"));
    }
    arquivo.seek(SeekFrom::Start(inicio))?;
    let mut dados = vec![0u8; tamanho];
    arquivo.read_exact(&mut dados)?;
    Ok(dados)
}
fn colocar16(b: &mut [u8], i: usize, v: u16) {
    b[i..i + 2].copy_from_slice(&v.to_le_bytes());
}
fn colocar32(b: &mut [u8], i: usize, v: u32) {
    b[i..i + 4].copy_from_slice(&v.to_le_bytes());
}
fn colocar64(b: &mut [u8], i: usize, v: u64) {
    b[i..i + 8].copy_from_slice(&v.to_le_bytes());
}
fn ler16(b: &[u8], i: usize) -> u16 {
    u16::from_le_bytes(b[i..i + 2].try_into().unwrap())
}
fn ler32(b: &[u8], i: usize) -> u32 {
    u32::from_le_bytes(b[i..i + 4].try_into().unwrap())
}
fn ler64(b: &[u8], i: usize) -> u64 {
    u64::from_le_bytes(b[i..i + 8].try_into().unwrap())
}

fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = u32::MAX;
    for &b in bytes {
        crc ^= u32::from(b);
        for _ in 0..8 {
            crc = (crc >> 1) ^ (0xedb8_8320 & 0u32.wrapping_sub(crc & 1));
        }
    }
    !crc
}

#[cfg(test)]
mod testes {
    #[test]
    fn falha_real_de_preparacao_exige_recusa_em_ordem_e_preserva_rede_antiga() {
        use super::{linha_falha_preparacao_presente, validar_log_falha_preparacao};
        let marcador = "[teste-falha-preparacao] ERRO_REAL_LOADER antiga=0x100000002 nova=0x200000002 motivo=ELF_INVALIDO";
        let valido = b"raiz> [teste-falha-preparacao] ERRO_REAL_LOADER antiga=0x100000002 nova=0x200000002 motivo=ELF_INVALIDO\n[microkernel] REINICIO_RECUSADO referencia=0x100000002 motivo=ELF de servico invalido\n[supervisor] OPERACAO_RECUSADA codigo=0xfffffffffffffff7 operacao=9 geracao=1\n";
        assert!(validar_log_falha_preparacao(valido).is_ok());
        assert!(linha_falha_preparacao_presente(valido, marcador));
        for invalido in [
            b"[microkernel] REINICIO_RECUSADO referencia=0x100000002 motivo=ELF de servico invalido\n[teste-falha-preparacao] ERRO_REAL_LOADER antiga=0x100000002 nova=0x200000002 motivo=ELF_INVALIDO\n[supervisor] OPERACAO_RECUSADA codigo=0xfffffffffffffff7 operacao=9 geracao=1\n".as_slice(),
            b"[teste-falha-preparacao] ERRO_REAL_LOADER antiga=0x100000002 nova=0x200000002 motivo=ELF_INVALIDO\n[teste-falha-preparacao] ERRO_REAL_LOADER antiga=0x100000002 nova=0x200000002 motivo=ELF_INVALIDO\n[microkernel] REINICIO_RECUSADO referencia=0x100000002 motivo=ELF de servico invalido\n[supervisor] OPERACAO_RECUSADA codigo=0xfffffffffffffff7 operacao=9 geracao=1\n",
            b"[teste-falha-preparacao] ERRO_REAL_LOADER antiga=0x100000002 nova=0x200000002 motivo=ELF_INVALIDO\n[microkernel] REINICIO_RECUSADO referencia=0x100000002 motivo=ELF de servico invalido\n[supervisor] OPERACAO_RECUSADA codigo=0xfffffffffffffff7 operacao=9 geracao=1\n[microkernel] SERVICO_REINICIADO id=2 antiga=0x100000002 nova=0x200000002\n",
            b"[teste-falha-preparacao] ERRO_REAL_LOADER antiga=0x100000002 nova=0x200000002 motivo=ELF_INVALIDO",
            b"raiz> eco [teste-falha-preparacao] ERRO_REAL_LOADER antiga=0x100000002 nova=0x200000002 motivo=ELF_INVALIDO\n[microkernel] REINICIO_RECUSADO referencia=0x100000002 motivo=ELF de servico invalido\n[supervisor] OPERACAO_RECUSADA codigo=0xfffffffffffffff7 operacao=9 geracao=1\n",
            b"raiz> reiniciar-rede [teste-falha-preparacao] ERRO_REAL_LOADER antiga=0x100000002 nova=0x200000002 motivo=ELF_INVALIDO\n[microkernel] REINICIO_RECUSADO referencia=0x100000002 motivo=ELF de servico invalido\n[supervisor] OPERACAO_RECUSADA codigo=0xfffffffffffffff7 operacao=9 geracao=1\n",
            b"raiz> [teste-falha-preparacao] ERRO_REAL_LOADER antiga=0x100000002 nova=0x200000002 motivo=ELF_INVALIDO",
            b"raiz> [teste-falha-preparacao] ERRO_REAL_LOADER antiga=0x100000002 nova=0x200000002 motivo=ELF_INVALIDO\nraiz> [teste-falha-preparacao] ERRO_REAL_LOADER antiga=0x100000002 nova=0x200000002 motivo=ELF_INVALIDO\n[microkernel] REINICIO_RECUSADO referencia=0x100000002 motivo=ELF de servico invalido\n[supervisor] OPERACAO_RECUSADA codigo=0xfffffffffffffff7 operacao=9 geracao=1\n",
        ] {
            assert!(validar_log_falha_preparacao(invalido).is_err());
        }
        assert!(!linha_falha_preparacao_presente(
            b"raiz> eco [teste-falha-preparacao] ERRO_REAL_LOADER antiga=0x100000002 nova=0x200000002 motivo=ELF_INVALIDO\n",
            marcador
        ));
        assert!(!linha_falha_preparacao_presente(
            b"raiz> reiniciar-rede [teste-falha-preparacao] ERRO_REAL_LOADER antiga=0x100000002 nova=0x200000002 motivo=ELF_INVALIDO\n",
            marcador
        ));
    }

    #[test]
    fn oraculo_irq_fifo_cheia_exige_coexistencia_trace_real_e_dreno_exato() {
        use super::validar_log_irq_fifo_cheia;
        fn log(preferir_irq_inicial: bool) -> String {
            fn emit_ipc(
                linhas: &mut Vec<String>,
                preferencia: u64,
                numero: u64,
                fila: u64,
                irq_pendente: u64,
            ) {
                let (origem, codigo, payload) = if numero <= 15 {
                    ("0x100000002", "0x7da", numero)
                } else {
                    ("0x100000003", "0x7db", 1)
                };
                linhas.push(format!("[teste-irq-fifo-cheia] SELETOR terminal_ref=0x100000001 preferir_irq={preferencia} ipc_validas={fila} fila={fila} irq_pendente={irq_pendente} escolhido=IPC origem={origem} codigo={codigo} a={payload}"));
            }
            let mut linhas = vec![
                "[teste-irq-fifo-cheia] REDE_FIFO quantidade=15 origem=0x100000002 ordem=1..15 irq_vazia=1".to_owned(),
                "[teste-irq-fifo-cheia] FIFO_CHEIA quantidade=16 rede=15 supervisor=1 irq_vazia=1 imr_irq4=0 CPL3".to_owned(),
                "[teste-irq-fifo-cheia] IRQ_PENDENTE fifo=16 irq_selecionavel=1 imr_irq4=1".to_owned(),
                "[teste-irq-fifo-cheia] BYTE_UART valor=0x41".to_owned(),
            ];
            if preferir_irq_inicial {
                linhas.push("[teste-irq-fifo-cheia] SELETOR terminal_ref=0x100000001 preferir_irq=1 ipc_validas=16 fila=16 irq_pendente=1 escolhido=IRQ seq=9".into());
                linhas.push("[teste-irq-fifo-cheia] IRQ_ENTREGUE seq=9".into());
                for n in 1..=15 {
                    emit_ipc(&mut linhas, if n == 1 { 0 } else { 1 }, n, 17 - n, 0);
                }
                emit_ipc(&mut linhas, 1, 16, 1, 0);
            } else {
                emit_ipc(&mut linhas, 0, 1, 16, 1);
                linhas.push("[teste-irq-fifo-cheia] SELETOR terminal_ref=0x100000001 preferir_irq=1 ipc_validas=15 fila=15 irq_pendente=1 escolhido=IRQ seq=9".into());
                linhas.push("[teste-irq-fifo-cheia] IRQ_ENTREGUE seq=9".into());
                for n in 2..=15 {
                    emit_ipc(&mut linhas, if n == 2 { 0 } else { 1 }, n, 17 - n, 0);
                }
                emit_ipc(&mut linhas, 1, 16, 1, 0);
            }
            linhas.push(
                "[teste-irq-fifo-cheia] DRENO_OK rede=15 supervisor=1 fifo=0 irq_seq=9 byte=0x41"
                    .into(),
            );
            linhas.push("[teste-irq-fifo-cheia] FIM CPL3".into());
            format!("{}\n", linhas.join("\n"))
        }
        let ipc_primeiro = log(false);
        let irq_primeiro = log(true);
        assert!(validar_log_irq_fifo_cheia(ipc_primeiro.as_bytes()).is_ok());
        assert!(validar_log_irq_fifo_cheia(irq_primeiro.as_bytes()).is_ok());
        assert!(validar_log_irq_fifo_cheia(
            format!("{}[teste-irq-fifo-cheia] FIFO_CHEIA quantidade=16 rede=15 supervisor=1 irq_vazia=1 imr_irq4=0 CPL3\n", ipc_primeiro).as_bytes()
        ).is_err());
        let sem_escolha = ipc_primeiro.replace(
            "[teste-irq-fifo-cheia] SELETOR terminal_ref=0x100000001 preferir_irq=1 ipc_validas=15 fila=15 irq_pendente=1 escolhido=IRQ seq=9\n",
            "",
        );
        assert!(validar_log_irq_fifo_cheia(sem_escolha.as_bytes()).is_err());
        let irq_confirmada_antes_do_trace = ipc_primeiro.replace(
            "[teste-irq-fifo-cheia] SELETOR terminal_ref=0x100000001 preferir_irq=1 ipc_validas=15 fila=15 irq_pendente=1 escolhido=IRQ seq=9\n[teste-irq-fifo-cheia] IRQ_ENTREGUE seq=9\n",
            "[teste-irq-fifo-cheia] IRQ_ENTREGUE seq=9\n[teste-irq-fifo-cheia] SELETOR terminal_ref=0x100000001 preferir_irq=1 ipc_validas=15 fila=15 irq_pendente=1 escolhido=IRQ seq=9\n",
        );
        assert!(validar_log_irq_fifo_cheia(irq_confirmada_antes_do_trace.as_bytes()).is_err());
        let byte_depois_do_trace = ipc_primeiro
            .replace("[teste-irq-fifo-cheia] BYTE_UART valor=0x41\n", "")
            .replace(
                "[teste-irq-fifo-cheia] IRQ_ENTREGUE seq=9\n",
                "[teste-irq-fifo-cheia] BYTE_UART valor=0x41\n[teste-irq-fifo-cheia] IRQ_ENTREGUE seq=9\n",
            );
        assert!(validar_log_irq_fifo_cheia(byte_depois_do_trace.as_bytes()).is_err());
        let ultima_trace = "[teste-irq-fifo-cheia] SELETOR terminal_ref=0x100000001 preferir_irq=1 ipc_validas=1 fila=1 irq_pendente=0 escolhido=IPC origem=0x100000003 codigo=0x7db a=1";
        let trace_depois_dreno = ipc_primeiro
            .replace(&format!("{ultima_trace}\n"), "")
            .replace(
                "[teste-irq-fifo-cheia] DRENO_OK rede=15 supervisor=1 fifo=0 irq_seq=9 byte=0x41\n",
                &format!("[teste-irq-fifo-cheia] DRENO_OK rede=15 supervisor=1 fifo=0 irq_seq=9 byte=0x41\n{ultima_trace}\n"),
            );
        assert!(validar_log_irq_fifo_cheia(trace_depois_dreno.as_bytes()).is_err());
        let eco_sem_real = ipc_primeiro
            .replace("[teste-irq-fifo-cheia] REDE_FIFO", "raiz> eco [teste-irq-fifo-cheia] REDE_FIFO")
            .replace("[teste-irq-fifo-cheia] REDE_FIFO quantidade=15 origem=0x100000002 ordem=1..15 irq_vazia=1\n", "");
        assert!(validar_log_irq_fifo_cheia(eco_sem_real.as_bytes()).is_err());
        assert!(validar_log_irq_fifo_cheia(
            format!("{ipc_primeiro}[teste-irq-fifo-cheia] IRQ_PENDENTE fifo=16").as_bytes()
        )
        .is_err());
    }

    #[test]
    fn sha256_aceita_formatos_certutil_e_sha256sum_sem_ruido() {
        use super::extrair_hash_sha256;
        let hash = "0123456789abcdef".repeat(4);
        let certutil = format!("SHA256 hash do arquivo:\n{hash}\nCertUtil: comando concluído.\n");
        assert_eq!(extrair_hash_sha256(&certutil, "certutil").unwrap(), hash);
        let sha256sum = format!("{hash}  artifact.img\n");
        assert_eq!(extrair_hash_sha256(&sha256sum, "sha256sum").unwrap(), hash);
        let sha256sum_binario = format!("{hash} *artifact.img\n");
        assert_eq!(
            extrair_hash_sha256(&sha256sum_binario, "sha256sum").unwrap(),
            hash
        );
        assert!(extrair_hash_sha256(&format!("ruido {hash}\n"), "sha256sum").is_err());
        assert!(extrair_hash_sha256(&format!("{hash} sufixo\n"), "certutil").is_err());
        assert!(extrair_hash_sha256(&format!("{hash} x artifact\n"), "sha256sum").is_err());
    }

    #[test]
    fn idle_rz024_correlaciona_fontes_e_rejeita_linhas_duplicadas_ou_parciais() {
        use super::{linha_serial_completa, validar_idle_rz024};
        let irq = b"[teste-rz024] JANELA_ARMADA cenario=irq4 referencia=0x100000001\n[teste-irq] BLOQUEADO\n[teste-rz024] IDLE_INICIO cenario=irq4 janela=1 entradas_idle=1 executaveis=0 terminal=0x100000001 rede=0x100000002 supervisor=0x100000003\n[teste-irq] IRQ4_CPL0\n[teste-rz024] IDLE_FIM cenario=irq4 janela=1 fonte=IRQ4 entradas_idle=1 tiques_irq0_idle=0 entradas_irq4_idle=1 mascara_irq4=1 selecionado=0x100000001\n[teste-irq] IRQ_ENTREGUE seq=9\n";
        assert!(validar_idle_rz024(irq, "IRQ4").is_ok());
        assert!(validar_idle_rz024(&[irq.as_slice(), b"raiz> "].concat(), "IRQ4").is_ok());
        assert!(
            validar_idle_rz024(&[irq.as_slice(), b"[teste-rz024] IDLE_FI"].concat(), "IRQ4")
                .is_err()
        );
        let prazo = b"[teste-rz024] JANELA_ARMADA cenario=prazo referencia=0x100000001\n[teste-ipc] RECEBER_BLOQUEADO id=1\n[teste-ipc] RECEBER_BLOQUEADO id=2\n[teste-prazo] BLOQUEADO id=3\n[teste-rz024] IDLE_INICIO cenario=prazo janela=1 entradas_idle=1 executaveis=0 terminal=0x100000001 rede=0x100000002 supervisor=0x100000003\n[teste-prazo-idle] IRQ0_CPL0\n[teste-rz024] IDLE_FIM cenario=prazo janela=1 fonte=IRQ0 entradas_idle=1 tiques_irq0_idle=2 entradas_irq4_idle=0 mascara_irq4=1 selecionado=0x100000003\n[teste-prazo] TIMEOUT id=3\n[teste-prazo-idle] RESPOSTA_CPL3\n";
        assert!(validar_idle_rz024(prazo, "IRQ0").is_ok());
        assert!(validar_idle_rz024(&[irq.as_slice(), irq.as_slice()].concat(), "IRQ4").is_err());
        assert!(validar_idle_rz024(&irq[..irq.len() - 1], "IRQ4").is_err());
        let fim_irq = "[teste-rz024] IDLE_FIM cenario=irq4 janela=1 fonte=IRQ4 entradas_idle=1 tiques_irq0_idle=0 entradas_irq4_idle=1 mascara_irq4=1 selecionado=0x100000001";
        assert!(linha_serial_completa(irq, fim_irq));
        assert!(!linha_serial_completa(fim_irq.as_bytes(), fim_irq));
        let sufixo = String::from_utf8(irq.to_vec())
            .unwrap()
            .replace("irq4=1 ", "irq4=10 ");
        assert!(validar_idle_rz024(sufixo.as_bytes(), "IRQ4").is_err());
        let eco = format!("eco {}", String::from_utf8_lossy(irq));
        assert!(validar_idle_rz024(eco.as_bytes(), "IRQ4").is_err());
        let sem_resposta = String::from_utf8(prazo.to_vec())
            .unwrap()
            .replace("[teste-prazo-idle] RESPOSTA_CPL3\n", "");
        assert!(validar_idle_rz024(sem_resposta.as_bytes(), "IRQ0").is_err());
        let identidade_errada = String::from_utf8(irq.to_vec())
            .unwrap()
            .replace("terminal=0x100000001", "terminal=0x100000004");
        assert!(validar_idle_rz024(identidade_errada.as_bytes(), "IRQ4").is_err());
        let fora_de_ordem = String::from_utf8(prazo.to_vec())
            .unwrap()
            .replace(
                "[teste-rz024] IDLE_INICIO cenario=prazo janela=1 entradas_idle=1 executaveis=0 terminal=0x100000001 rede=0x100000002 supervisor=0x100000003\n",
                "MARCADOR_TEMPORARIO\n",
            )
            .replace(
                "[teste-rz024] IDLE_FIM cenario=prazo janela=1 fonte=IRQ0 entradas_idle=1 tiques_irq0_idle=2 entradas_irq4_idle=0 mascara_irq4=1 selecionado=0x100000003\n",
                "[teste-rz024] IDLE_INICIO cenario=prazo janela=1 entradas_idle=1 executaveis=0 terminal=0x100000001 rede=0x100000002 supervisor=0x100000003\n",
            )
            .replace(
                "MARCADOR_TEMPORARIO\n",
                "[teste-rz024] IDLE_FIM cenario=prazo janela=1 fonte=IRQ0 entradas_idle=1 tiques_irq0_idle=2 entradas_irq4_idle=0 mascara_irq4=1 selecionado=0x100000003\n",
            );
        assert!(validar_idle_rz024(fora_de_ordem.as_bytes(), "IRQ0").is_err());
    }

    #[test]
    fn espera_evento_irq_morte_exige_prefixo_tokens_e_linha_completa() {
        use super::evento_completo_presente;
        let marcador = "[teste-irq-morte] TIMER_AMOSTRAS";
        let campos = ["quantidade=50", "mascara_ok=50"];
        assert!(evento_completo_presente(
            b"\r\n[teste-irq-morte] TIMER_AMOSTRAS quantidade=50 mascara_ok=50\n",
            marcador,
            &campos
        ));
        assert!(!evento_completo_presente(
            b"raiz> [teste-irq-morte] TIMER_AMOSTRAS quantidade=50 mascara_ok=50\n",
            marcador,
            &campos
        ));
        assert!(!evento_completo_presente(
            b"[teste-irq-morte] TIMER_AMOSTRAS quantidade=50 mascara_ok=50",
            marcador,
            &campos
        ));
        assert!(!evento_completo_presente(
            b"[teste-irq-morte] TIMER_AMOSTRAS quantidade=500 mascara_ok=50\n",
            marcador,
            &campos
        ));
        assert!(!evento_completo_presente(
            b"[teste-irq-morte] TIMER_AMOSTRAS quantidade=50 mascara_ok=500\n",
            marcador,
            &campos
        ));
    }

    #[test]
    fn oraculo_irq_morte_correlaciona_entrega_revogacao_estado_e_amostras_limitadas() {
        use super::validar_log_irq_morte;
        let valido = concat!(
            "[teste] TERMINAL_PRONTO\n",
            "[teste-irq-morte] ARMADA CPL3\n",
            "[teste-irq-morte] IRQ4_RECEBIDA seq=9 byte=0x41 CPL3\n",
            "[teste-irq-morte] IRQ4_REVOGACAO_INICIO referencia=0x100000001 seq=9 mascara=1\n",
            "[teste-irq-morte] IRQ4_REVOGADA referencia=0 mascara=1 tique=200\n",
            "[microkernel] SERVICO_FALHOU id=1 vetor=6 erro=0 RIP=0 CR2=0 referencia=0x100000001\n",
            "[teste-irq-morte] SUPERVISOR_REAGIU_FALHOU referencia=0x100000001\n",
            "[teste-irq-morte] TIMER_AMOSTRAS quantidade=50 tique_inicial=201 tique_final=250 desde_falha=50 mascara_ok=50 associacao_ausente_ok=50 entradas_irq4=0\n",
        );
        assert!(validar_log_irq_morte(valido).is_ok());
        assert!(validar_log_irq_morte(&valido.replace(
            "[teste-irq-morte] IRQ4_REVOGADA referencia=0 mascara=1 tique=200\n",
            ""
        ))
        .is_err());
        assert!(
            validar_log_irq_morte(&valido.replace("seq=9 mascara=1", "seq=10 mascara=1")).is_err()
        );
        assert!(
            validar_log_irq_morte(&valido.replace("seq=9 mascara=1", "seq=9 mascara=0")).is_err()
        );
        assert!(validar_log_irq_morte(&valido.replace("byte=0x41", "byte=0x42")).is_err());
        assert!(validar_log_irq_morte(
            &valido.replace("SUPERVISOR_REAGIU_FALHOU", "SUPERVISOR_CONSULTA_FALHOU")
        )
        .is_err());
        assert!(validar_log_irq_morte(&valido.replace("quantidade=50", "quantidade=49")).is_err());
        assert!(validar_log_irq_morte(&valido.replace("quantidade=50", "quantidade=500")).is_err());
        assert!(validar_log_irq_morte(&valido.replace("mascara_ok=50", "mascara_ok=50x")).is_err());
        assert!(validar_log_irq_morte(&valido.replace(
            "mascara_ok=50 associacao_ausente_ok=50",
            "mascara_ok=50 mascara_ok=50 associacao_ausente_ok=50"
        ))
        .is_err());
        assert!(validar_log_irq_morte(&format!(
            "{valido}[teste-irq-morte] TIMER_AMOSTRAS quantidade=50 duplicada\n"
        ))
        .is_err());
        assert!(validar_log_irq_morte(&valido[..valido.len() - 1]).is_err());
        assert!(
            validar_log_irq_morte("raiz> eco [teste-irq-morte] IRQ4_RECEBIDA seq=9 CPL3\n")
                .is_err()
        );
    }

    #[test]
    fn oraculo_cancelamento_exige_correlacao_ordem_inversa_e_estado_terminal() {
        use super::validar_log_cancelamento_aplicacao;
        let valido = concat!(
            "[teste-cancelamento] ENVIO_ACEITO id=0x2401\n",
            "[teste-cancelamento] ENVIO_ACEITO id=0x2402\n",
            "[teste-cancelamento] SERVIDOR_RECEBEU ids=0x2401,0x2402 CPL3\n",
            "[teste-cancelamento] CANCELADO_LOCAL id=0x2401\n",
            "[teste-cancelamento] RESPOSTA_ATIVA id=0x2402 valor=0x2222\n",
            "[teste-cancelamento] RESPOSTA_TARDIA_IGNORADA id=0x2401 estado=cancelada\n",
            "[teste-cancelamento] CANCELAMENTO_PRESERVADO\n",
        );
        assert!(validar_log_cancelamento_aplicacao(valido).is_ok());
        assert!(validar_log_cancelamento_aplicacao(&valido.replace(
            "[teste-cancelamento] RESPOSTA_TARDIA_IGNORADA id=0x2401 estado=cancelada\n",
            ""
        ))
        .is_err());
        assert!(validar_log_cancelamento_aplicacao(&valido.replace(
            "RESPOSTA_ATIVA id=0x2402 valor=0x2222",
            "RESPOSTA_ATIVA id=0x2401 valor=0x1111"
        ))
        .is_err());
        assert!(validar_log_cancelamento_aplicacao(&valido.replace(
            "[teste-cancelamento] CANCELADO_LOCAL id=0x2401\n",
            "[teste-cancelamento] CANCELADO_LOCAL id=0x2401\n[teste-cancelamento] CANCELADO_LOCAL id=0x2401\n"
        ))
        .is_err());
        assert!(validar_log_cancelamento_aplicacao(&format!(
            "{valido}[teste-cancelamento] CANCELAMENTO_PRESERVADO"
        ))
        .is_err());
        assert!(validar_log_cancelamento_aplicacao(
            "raiz> eco [teste-cancelamento] CANCELAMENTO_PRESERVADO\n"
        )
        .is_err());
    }

    #[test]
    fn oraculo_irq_rejeita_classe_ipc_impostora_e_exige_confirmacao_da_irq_original() {
        use super::validar_log_teste_irq;
        let valido = concat!(
            "[teste-irq] IRQ_ENTREGUE seq=7\n",
            "[teste-irq] IPC_IMPOSTORA_REJEITADA seq=7 codigo=3 retorno=0xfffffffffffffffe CPL3\n",
            "[teste-irq] JANELA CPL3\n",
            "[teste-irq] MASCARA_SEM_TEMPESTADE CPL3\n",
            "[teste-irq] IRQ_ORIGINAL_CONFIRMADA seq=7 CPL3\n",
            "[teste-irq] REARMADA CPL3\n",
            "[teste-irq] ENTREGA_CONFIRMACAO_REARME CPL3\n",
        );
        assert!(validar_log_teste_irq(valido).is_ok());
        assert!(validar_log_teste_irq(&valido.replace("seq=7 CPL3", "seq=8 CPL3")).is_err());
        assert!(validar_log_teste_irq(&valido.replace("codigo=3", "codigo=1")).is_err());
        assert!(
            validar_log_teste_irq(&valido.replace("retorno=0xfffffffffffffffe", "retorno=0"))
                .is_err()
        );
        assert!(validar_log_teste_irq(&valido.replace(
            "[teste-irq] IPC_IMPOSTORA_REJEITADA seq=7 codigo=3 retorno=0xfffffffffffffffe CPL3\n",
            ""
        ))
        .is_err());
        assert!(validar_log_teste_irq(&valido.replace(
            "[teste-irq] IPC_IMPOSTORA_REJEITADA seq=7 codigo=3 retorno=0xfffffffffffffffe CPL3\n",
            "[teste-irq] IPC_IMPOSTORA_REJEITADA seq=7 codigo=3 retorno=0xfffffffffffffffe CPL3\n[teste-irq] IPC_IMPOSTORA_REJEITADA seq=7 codigo=3 retorno=0xfffffffffffffffe CPL3\n"
        ))
        .is_err());
        assert!(validar_log_teste_irq(&format!(
            "{valido}[teste-irq] IRQ_ORIGINAL_CONFIRMADA seq=7 CPL3"
        ))
        .is_err());
        assert!(validar_log_teste_irq(&valido.replace(
            "[teste-irq] MASCARA_SEM_TEMPESTADE CPL3\n[teste-irq] IRQ_ORIGINAL_CONFIRMADA",
            "[teste-irq] IRQ_ORIGINAL_CONFIRMADA"
        ))
        .is_err());
    }

    #[test]
    fn oraculo_rx_orcamento_exige_lote_cessao_continuacao_e_linhas_completas() {
        use super::validar_log_rx_orcamento;
        let valido = concat!(
            "raiz> eco [teste-rx-orcamento] LOTE descritores=16 teste=16 mascara=0xffff esgotado=1\n",
            "[teste-rx-orcamento] ARMADA quantidade=17 rx_base=1 quadros_base=0 CPL3\n",
            "[teste-rx-orcamento] DD_PRONTOS descritores=17 indice_rx=31\n",
            "[teste-rx-orcamento] LOTE descritores=16 teste=16 mascara=0x1fffe esgotado=1\n",
            "[teste-rx-orcamento] CEDER_RETORNOU\n",
            "[teste-rx-orcamento] CONTINUOU descritores=1 teste=1 mascara_nova=0x1 mascara_total=0x1ffff esgotado=0\n",
        );
        assert!(
            validar_log_rx_orcamento(valido).is_ok(),
            "{:?}",
            validar_log_rx_orcamento(valido)
        );
        assert!(validar_log_rx_orcamento(&valido.replace(
            "[teste-rx-orcamento] DD_PRONTOS descritores=17 indice_rx=31\n",
            ""
        ))
        .is_err());
        assert!(validar_log_rx_orcamento(&valido.replace(
            "DD_PRONTOS descritores=17 indice_rx=31",
            "DD_PRONTOS descritores=17 indice_rx=32"
        ))
        .is_err());
        let outra_permutacao = valido
            .replace("mascara=0x1fffe", "mascara=0x1fffd")
            .replace("mascara_nova=0x1", "mascara_nova=0x2");
        assert!(validar_log_rx_orcamento(&outra_permutacao).is_ok());
        let identificador_duplicado = valido.replace("mascara=0x1fffe", "mascara=0xffff");
        assert!(validar_log_rx_orcamento(&identificador_duplicado).is_err());
        assert!(validar_log_rx_orcamento(
            &valido.replace("[teste-rx-orcamento] CEDER_RETORNOU\n", "")
        )
        .is_err());
        assert!(validar_log_rx_orcamento(&valido.replace(
            "[teste-rx-orcamento] LOTE descritores=16 teste=16 mascara=0x1fffe esgotado=1\n[teste-rx-orcamento] CEDER_RETORNOU",
            "[teste-rx-orcamento] CEDER_RETORNOU\n[teste-rx-orcamento] LOTE descritores=16 teste=16 mascara=0x1fffe esgotado=1"
        ))
        .is_err());
        assert!(
            validar_log_rx_orcamento(&format!("{valido}[teste-rx-orcamento] CEDER_RETORNOU"))
                .is_err()
        );
    }

    #[test]
    fn oraculo_rx_quota_correlaciona_trace_da_quota_supervisor_e_retry() {
        use super::validar_log_rx_quota;
        let valido = concat!(
            "[teste-rx-quota] ARMADA ids=1..18 rede=CPL3 supervisor=CPL3 quota=15\n",
            "[teste-rx-quota] FIFO_REDE_PRONTA quantidade=15 origem=0x100000002\n",
            "[teste-rx-quota] FILA_CHEIA origem=0x100000002 destino=0x100000001 id=16 codigo=0x7ba retorno=0xfffffffffffffffd fila=15 quota_rede=15 CPL3\n",
            "[teste-rx-quota] FIFO_CHEIA rede=15 supervisor=1 total=16 CPL3\n",
            "[teste-rx-quota] RETRY_OK id=16 quadros_driver=16 mascara=0xffff retries=1 driver_invalido=0\n",
            "[teste-rx-quota] CONTINUACAO_PRONTA ids=17..18\n",
            "[teste-rx-quota] RESULTADO rx=18 ipc_rede=18 controle_supervisor=1 fila_cheia=1 retry_id=16 ordem=1..18 driver_invalido=0 CPL3\n",
        );
        assert!(validar_log_rx_quota(valido.as_bytes()).is_ok());
        assert!(validar_log_rx_quota(
            valido
                .replace(
                    "[teste-rx-quota] FILA_CHEIA origem=0x100000002",
                    "raiz> [teste-rx-quota] FILA_CHEIA origem=0x100000002"
                )
                .as_bytes()
        )
        .is_ok());

        assert!(
            validar_log_rx_quota(valido.replace("quota_rede=15", "quota_rede=150").as_bytes())
                .is_err()
        );
        assert!(validar_log_rx_quota(
            valido
                .replace("driver_invalido=0", "driver_invalido=1")
                .as_bytes()
        )
        .is_err());
        assert!(validar_log_rx_quota(
            valido
                .replace("id=16 codigo=0x7ba", "id=160 codigo=0x7ba")
                .as_bytes()
        )
        .is_err());
        assert!(validar_log_rx_quota(
            &valido
                .replace(
                    "origem=0x100000002 destino=0x100000001",
                    "origem=0x200000002 destino=0x100000001"
                )
                .into_bytes()
        )
        .is_err());
        assert!(validar_log_rx_quota(
            &valido
                .replace(
                    "[teste-rx-quota] FIFO_CHEIA rede=15 supervisor=1 total=16 CPL3\n",
                    ""
                )
                .into_bytes()
        )
        .is_err());
        assert!(validar_log_rx_quota(&valido.replace(
            "[teste-rx-quota] FIFO_CHEIA rede=15 supervisor=1 total=16 CPL3\n",
            "[teste-rx-quota] FIFO_CHEIA rede=15 supervisor=1 total=16 CPL3\n[teste-rx-quota] FIFO_CHEIA rede=15 supervisor=1 total=16 CPL3\n"
        ).into_bytes())
        .is_err());
        assert!(validar_log_rx_quota(&valido.replace(
            "[teste-rx-quota] FIFO_CHEIA rede=15 supervisor=1 total=16 CPL3\n",
            "[teste-rx-quota] FIFO_CHEIA rede=15 supervisor=1 total=16 CPL3\n[teste-rx-quota] FILA_CHEIA origem=0x100000002 destino=0x100000001 id=16 codigo=0x7ba retorno=0xfffffffffffffffd fila=150 quota_rede=15 CPL3\n"
        ).into_bytes())
        .is_err());
        assert!(validar_log_rx_quota(&valido.replace(
            "[teste-rx-quota] FIFO_CHEIA rede=15 supervisor=1 total=16 CPL3\n",
            "[teste-rx-quota] FIFO_CHEIA rede=15 supervisor=1 total=16 CPL3\n[microkernel] SERVICO_FALHOU id=2 vetor=6\n"
        ).into_bytes())
        .is_err());
        assert!(validar_log_rx_quota(
            b"[teste-rx-quota] ARMADA ids=1..18 rede=CPL3 supervisor=CPL3 quota=15"
        )
        .is_err());
        assert!(validar_log_rx_quota(
            &valido
                .replace(
                    "[teste-rx-quota] ARMADA",
                    "raiz> eco [teste-rx-quota] ARMADA"
                )
                .into_bytes()
        )
        .is_err());
        assert!(validar_log_rx_quota(
            &valido
                .replace(
                    "[teste-rx-quota] FILA_CHEIA origem=0x100000002",
                    "prompt extra raiz> [teste-rx-quota] FILA_CHEIA origem=0x100000002"
                )
                .into_bytes()
        )
        .is_err());
    }

    #[test]
    fn barreira_rx_quota_aceita_somente_prompt_literal_antes_da_linha_completa() {
        use super::linha_rx_quota_completa;
        assert!(linha_rx_quota_completa(
            b"raiz> [teste-rx-quota] FILA_CHEIA id=16\n",
            "[teste-rx-quota] FILA_CHEIA"
        ));
        assert!(!linha_rx_quota_completa(
            b"echo raiz> [teste-rx-quota] FILA_CHEIA id=16\n",
            "[teste-rx-quota] FILA_CHEIA"
        ));
        assert!(!linha_rx_quota_completa(
            b"raiz> [teste-rx-quota] FILA_CHEIA id=16",
            "[teste-rx-quota] FILA_CHEIA"
        ));
    }

    #[test]
    fn oraculo_irq_ipc_exige_linhas_completas_e_nao_aceita_eco_como_ipc() {
        use super::validar_log_irq_ipc;
        let mut log =
            String::from("raiz> eco [teste-irq-ipc] IPC_DURANTE_TRABALHO_SERIAL seq=99\n");
        for _ in 0..8 {
            log.push_str("[teste-irq-ipc] BYTE_IRQ\n");
        }
        log.push_str("[teste-irq-ipc] CARGA_PRONTA CPL3\n");
        log.push_str(
            "[teste-irq-ipc] IRQ4 seq=7\n[teste-irq-ipc] RETENCAO_FIM\n[teste-irq-ipc] IRQ4 seq=8\n",
        );
        let mut medicao = String::new();
        for _ in 0..8 {
            medicao.push_str("[teste-irq-ipc] BYTE_IRQ\n");
        }
        for sequencia in 1..=8 {
            let classe = if sequencia == 1 {
                "IPC_DURANTE_TRABALHO_SERIAL"
            } else {
                "IPC_SEM_TRABALHO_SERIAL"
            };
            medicao.push_str(&format!("[teste-irq-ipc] {classe} seq={sequencia}\n"));
        }
        log.push_str(&medicao);
        assert!(validar_log_irq_ipc(&log).is_ok());

        let eco_apenas = "raiz> eco [teste-irq-ipc] IPC_DURANTE_TRABALHO_SERIAL seq=1\n";
        assert!(validar_log_irq_ipc(eco_apenas).is_err());

        let incompleto = format!("{log}[teste-irq-ipc] IPC_DURANTE_TRABALHO_SERIAL seq=9");
        assert!(validar_log_irq_ipc(&incompleto).is_err());

        let sem_bytes_medidos = format!(
            "{}{}",
            "[teste-irq-ipc] BYTE_IRQ\n".repeat(8),
            "[teste-irq-ipc] CARGA_PRONTA CPL3\n[teste-irq-ipc] IRQ4 seq=7\n[teste-irq-ipc] RETENCAO_FIM\n[teste-irq-ipc] IRQ4 seq=8\n"
        ) + &medicao
            .replace("[teste-irq-ipc] BYTE_IRQ\n", "");
        assert!(validar_log_irq_ipc(&sem_bytes_medidos).is_err());

        let irq_com_lixo = log.replace("IRQ4 seq=7", "IRQ4 seq=7lixo");
        assert!(validar_log_irq_ipc(&irq_com_lixo).is_err());
    }

    #[test]
    fn oraculo_seletor_irq_ipc_exige_duas_fontes_e_alternancia_real() {
        use super::validar_log_seletor_irq_ipc;
        let valido = concat!(
            "raiz> eco [teste-seletor-irq-ipc] SELETOR falso\n",
            "[teste-seletor-irq-ipc] CARGA_PRONTA CPL3\n",
            "[teste-seletor-irq-ipc] IRQ4_PENDENTE mascara=1\n",
            "[teste-seletor-irq-ipc] BYTE_A_LIDO\n",
            "[teste-seletor-irq-ipc] AMBAS_PENDENTES irq=1 ipc=2 mascara=1 preferir_irq=0\n",
            "[teste-seletor-irq-ipc] SELETOR terminal_ref=0x100000001 preferir_irq=0 irq_pendente=1 irq_ref_atual=1 entregue=0 ipc_validas=2 escolhido=IPC origem=0x200000002 origem_atual=1 codigo=0x7e9 a=1\n",
            "[teste-seletor-irq-ipc] PRIMEIRA_IPC origem=0x200000002 codigo=0x7e9 id=1\n",
            "[teste-seletor-irq-ipc] SELETOR terminal_ref=0x100000001 preferir_irq=1 irq_pendente=1 irq_ref_atual=1 entregue=0 ipc_validas=1 escolhido=IRQ seq=9\n",
            "[teste-seletor-irq-ipc] IRQ4_CONFIRMADA seq=9\n",
            "[teste-seletor-irq-ipc] IPC_RESTANTE origem=0x200000002 codigo=0x7e9 id=2\n",
            "[teste-seletor-irq-ipc] FIM CPL3\n",
        );
        assert!(validar_log_seletor_irq_ipc(valido).is_ok());
        assert!(
            validar_log_seletor_irq_ipc(&valido.replace("irq_pendente=1", "irq_pendente=0"))
                .is_err()
        );
        assert!(validar_log_seletor_irq_ipc(
            &valido.replace("[teste-seletor-irq-ipc] IRQ4_PENDENTE mascara=1\n", "")
        )
        .is_err());
        assert!(
            validar_log_seletor_irq_ipc(&valido.replace("preferir_irq=1", "preferir_irq=0"))
                .is_err()
        );
        assert!(validar_log_seletor_irq_ipc(
            &valido.replace("entregue=0 ipc_validas=1", "entregue=0 ipc_validas=0")
        )
        .is_err());
        assert!(validar_log_seletor_irq_ipc(
            &valido.replace("codigo=0x7e9 a=1", "codigo=0x7e9 a=2")
        )
        .is_err());
        assert!(validar_log_seletor_irq_ipc(&valido.replace(
            "PRIMEIRA_IPC origem=0x200000002 codigo=0x7e9 id=1",
            "PRIMEIRA_IPC origem=0x200000003 codigo=0x7e9 id=1"
        ))
        .is_err());
        assert!(validar_log_seletor_irq_ipc(&valido.replace(
            "terminal_ref=0x100000001 preferir_irq=1",
            "terminal_ref=0x100000002 preferir_irq=1"
        ))
        .is_err());
        assert!(validar_log_seletor_irq_ipc(&valido.replace("seq=9", "seq=0")).is_err());
        assert!(validar_log_seletor_irq_ipc(
            &valido.replace("IPC_RESTANTE origem=0x200000002 codigo=0x7e9 id=2\n", "")
        )
        .is_err());
        assert!(validar_log_seletor_irq_ipc(&valido.replace(
            "IPC_RESTANTE origem=0x200000002 codigo=0x7e9 id=2",
            "IPC_RESTANTE origem=0x200000002 codigo=0x789 id=2"
        ))
        .is_err());
        assert!(
            validar_log_seletor_irq_ipc(&format!("{valido}[teste-seletor-irq-ipc] FIM CPL3"))
                .is_err()
        );
        assert!(validar_log_seletor_irq_ipc(&format!(
            "{valido}[teste-seletor-irq-ipc] SELETOR terminal_ref=0x100000001 preferir_irq=1 irq_pendente=1 irq_ref_atual=1 entregue=0 ipc_validas=1 escolhido=IRQ seq=10\n"
        ))
        .is_err());
        assert!(validar_log_seletor_irq_ipc(&format!(
            "{valido}[teste-seletor-irq-ipc] SELETOR terminal_ref=0x100000001 preferir_irq=1 irq_pendente=1 irq_ref_atual=1 entregue=0 ipc_validas=1 escolhido=IRQ seq=10"
        ))
        .is_err());
    }

    #[test]
    fn ack_fifo_rz022_exige_linhas_completas_identidade_e_ordem() {
        use super::sequencias_fifo_rz022;
        let mut log = String::new();
        for sequencia in 1..=32 {
            log.push_str(&format!(
                "[teste-rz022-rede] FIFO_ACK seq={sequencia} referencia=0x100000002 tique={}\n",
                sequencia + 100
            ));
        }
        assert_eq!(
            sequencias_fifo_rz022(log.as_bytes()),
            Some((1..=32).collect())
        );
        assert_eq!(
            sequencias_fifo_rz022(
                b"raiz> eco [teste-rz022-rede] FIFO_ACK seq=1 referencia=0x100000002 tique=101\n"
            ),
            Some(vec![])
        );
        let invertido = log.replace(
            "FIFO_ACK seq=1 referencia=0x100000002 tique=101\n[teste-rz022-rede] FIFO_ACK seq=2 referencia=0x100000002 tique=102",
            "FIFO_ACK seq=2 referencia=0x100000002 tique=102\n[teste-rz022-rede] FIFO_ACK seq=1 referencia=0x100000002 tique=101",
        );
        assert_ne!(
            sequencias_fifo_rz022(invertido.as_bytes()),
            Some((1..=32).collect())
        );
        assert_eq!(
            sequencias_fifo_rz022(
                b"[teste-rz022-rede] FIFO_ACK seq=1 referencia=0x200000002 tique=101\n"
            ),
            None
        );
    }

    #[test]
    fn backoff_rz022_respeita_ticks_modulares_e_horizonte() {
        use super::tique_após_prazo_rz022;
        assert!(tique_após_prazo_rz022(100, 200, 100));
        assert!(tique_após_prazo_rz022(u64::MAX - 4, 5, 10));
        assert!(!tique_após_prazo_rz022(100, 199, 100));
        assert!(!tique_após_prazo_rz022(100, 99, 1));
    }

    #[test]
    fn marcadores_de_vida_exigem_ordem_linhas_completas_e_nao_aceitam_eco() {
        use super::{linha_corresponde, linhas_ordenadas};
        assert!(linha_corresponde(
            "raiz> [teste-vida] BLOQUEADO\r\n",
            "[teste-vida] BLOQUEADO"
        ));
        assert!(!linha_corresponde(
            "raiz> eco [teste-vida] BLOQUEADO\n",
            "[teste-vida] BLOQUEADO"
        ));
        assert!(!linha_corresponde(
            "[teste-vida] BLOQUEADO",
            "[teste-vida] BLOQUEADO"
        ));
        assert!(linhas_ordenadas(
            b"[teste-vida-servico] ETAPA 3 CPL3\n[teste-vida-servico] ETAPA 4 CPL3\n",
            "[teste-vida-servico] ETAPA 3 CPL3",
            "[teste-vida-servico] ETAPA 4 CPL3"
        ));
        assert!(!linhas_ordenadas(
            b"[teste-vida-servico] ETAPA 4 CPL3\n[teste-vida-servico] ETAPA 3 CPL3\n",
            "[teste-vida-servico] ETAPA 3 CPL3",
            "[teste-vida-servico] ETAPA 4 CPL3"
        ));
    }
    #[test]
    fn recusa_completa_delimita_timeout_posterior() {
        use super::{fim_linha, tique_timeout_ciclo};
        let recusa = "[supervisor] RECURSOS_ESGOTADOS operacao=9 geracao=4";
        let dados = b"[teste-ciclo-espera] TIMEOUT referencia=0x400000002 tiques=100\n[supervisor] RECURSOS_ESGOTADOS operacao=9 geracao=4\n";
        let fim = fim_linha(dados, recusa).unwrap();
        assert_eq!(tique_timeout_ciclo(&dados[fim..], 0x400000002), None);
        assert_eq!(
            fim_linha(
                b"eco [supervisor] RECURSOS_ESGOTADOS operacao=9 geracao=4\n",
                recusa
            ),
            None
        );
        assert_eq!(
            fim_linha(
                b"[supervisor] RECURSOS_ESGOTADOS operacao=9 geracao=4",
                recusa
            ),
            None
        );
        let valido = b"[supervisor] RECURSOS_ESGOTADOS operacao=9 geracao=4\n[teste-ciclo-espera] TIMEOUT referencia=0x400000002 tiques=100\n";
        let fim = fim_linha(valido, recusa).unwrap();
        assert_eq!(tique_timeout_ciclo(&valido[fim..], 0x400000002), Some(100));
    }
    #[test]
    fn timeouts_ciclo_contam_so_instancias_de_rede_em_linhas_completas() {
        use super::timeouts_rede_ciclo;
        let serial = b"[teste-ciclo-espera] TIMEOUT referencia=0x100000001 tiques=3\nraiz> [teste-ciclo-espera] TIMEOUT referencia=0x400000002 tiques=100\r\nraiz> eco [teste-ciclo-espera] TIMEOUT referencia=0x300000002 tiques=50\n[teste-ciclo-espera] TIMEOUT referencia=0x200000002 tiques=40";
        assert_eq!(timeouts_rede_ciclo(serial), vec![(0x400000002, 100)]);
    }
    #[test]
    fn janela_timeout_gen4_fecha_no_primeiro_evento_e_exige_pos_recusa() {
        use super::janela_timeout_rede_ciclo;
        let recusa = b"[supervisor] RECURSOS_ESGOTADOS operacao=9 geracao=4\n";
        let anterior_a_gen4 = b"[teste-ciclo-espera] TIMEOUT referencia=0x300000002 tiques=90\n";
        let inicio_gen4 = anterior_a_gen4.len();
        let inicializacao_gen4 = b"[supervisor] REDE_REINICIADA geracao_nova=4\n";
        let serial = [
            anterior_a_gen4.as_slice(),
            inicializacao_gen4,
            recusa,
            b"raiz> [teste-ciclo-espera] TIMEOUT referencia=0x400000002 tiques=100\r\n",
            b"[teste-ciclo-espera] TIMEOUT referencia=0x400000002 tiques=110\n",
        ]
        .concat();
        let apos_recusa = inicio_gen4 + inicializacao_gen4.len() + recusa.len();
        assert_eq!(
            janela_timeout_rede_ciclo(&serial, inicio_gen4, apos_recusa, 0x400000002, 100),
            Some(vec![(0x400000002, 100)])
        );

        let gen4_timeout_antes_recusa = [
            anterior_a_gen4.as_slice(),
            inicializacao_gen4,
            b"[teste-ciclo-espera] TIMEOUT referencia=0x400000002 tiques=100\n",
            recusa,
            b"[teste-ciclo-espera] TIMEOUT referencia=0x400000002 tiques=100\n",
        ]
        .concat();
        let apos_recusa = inicio_gen4
            + inicializacao_gen4.len()
            + b"[teste-ciclo-espera] TIMEOUT referencia=0x400000002 tiques=100\n".len()
            + recusa.len();
        assert_eq!(
            janela_timeout_rede_ciclo(
                &gen4_timeout_antes_recusa,
                inicio_gen4,
                apos_recusa,
                0x400000002,
                100
            ),
            None
        );

        let geracao_anterior = [
            b"[teste-ciclo-espera] TIMEOUT referencia=0x300000002 tiques=90\n".as_slice(),
            recusa,
            b"[teste-ciclo-espera] TIMEOUT referencia=0x400000002 tiques=100\n",
        ]
        .concat();
        let apos_recusa = geracao_anterior
            .windows(recusa.len())
            .position(|janela| janela == recusa)
            .unwrap()
            + recusa.len();
        assert_eq!(
            janela_timeout_rede_ciclo(&geracao_anterior, 0, apos_recusa, 0x400000002, 100),
            Some(vec![(0x300000002, 90), (0x400000002, 100)])
        );
    }

    #[test]
    fn snapshot_driver_gen4_exige_identidade_estado_estavel_e_ordem() {
        use super::{snapshot_driver_ciclo, validar_snapshot_driver_gen4};
        let valido = concat!(
            "[teste-ciclo-espera] BLOQUEADA referencia=0x400000002 prazo=3\n",
            "[teste-ciclo-espera] TIMEOUT referencia=0x400000002 tiques=3\n",
            "[teste-ciclo-espera] DRIVER_ATIVO referencia=0x400000002 mmio=0x81040000 arena=0x24bf000 rx_idx=3 tx_idx=2 rx=12 tx=8 CPL3\n",
            "[teste-ciclo-espera] BLOQUEADA referencia=0x400000002 prazo=1000\n",
            "[supervisor] RECURSOS_ESGOTADOS operacao=9 geracao=4\n",
            "[teste-ciclo-espera] TIMEOUT referencia=0x400000002 tiques=1000\n",
            "[teste-ciclo-espera] DRIVER_PRESERVADO referencia=0x400000002 mmio=0x81040000 arena=0x24bf000 rx_idx=3 tx_idx=2 rx=12 tx=8 igual=1 CPL3\n",
            "[teste-ciclo-espera] BLOQUEADA referencia=0x400000002 prazo=2000\n",
            "[teste-ciclo-espera] TIMEOUT referencia=0x400000002 tiques=2000\n",
        );
        assert!(validar_snapshot_driver_gen4(valido.as_bytes()).is_ok());
        assert!(snapshot_driver_ciclo(valido.as_bytes(), "DRIVER_ATIVO", 0x400000002).is_some());
        assert!(validar_snapshot_driver_gen4(
            valido
                .replacen("arena=0x24bf000", "arena=0x24bf008", 1)
                .as_bytes()
        )
        .is_err());
        assert!(validar_snapshot_driver_gen4(
            valido
                .replace(
                    "[teste-ciclo-espera] DRIVER_PRESERVADO referencia=",
                    "[teste-ciclo-espera] DRIVER_ATIVO referencia=0x400000002 mmio=0x81040000 arena=0x24bf000 rx_idx=3 tx_idx=2 rx=12 tx=8 CPL3\n[teste-ciclo-espera] DRIVER_PRESERVADO referencia=",
                )
                .as_bytes()
        )
        .is_err());
        assert!(validar_snapshot_driver_gen4(
            valido
                .replace(
                    "[supervisor] RECURSOS_ESGOTADOS operacao=9 geracao=4\n",
                    "[teste-ciclo-espera] TIMEOUT referencia=0x400000002 tiques=1000\n[supervisor] RECURSOS_ESGOTADOS operacao=9 geracao=4\n",
                )
                .as_bytes()
        )
        .is_err());
        assert!(snapshot_driver_ciclo(
            b"[teste-ciclo-espera] DRIVER_ATIVO referencia=0x400000002 mmio=0x81040000",
            "DRIVER_ATIVO",
            0x400000002
        )
        .is_none());
    }
    #[test]
    fn prazo_de_referencia_exige_linha_completa_e_identidade_exata() {
        use super::prazo_ciclo;
        let linha = b"raiz> [teste-ciclo-espera] BLOQUEADA referencia=0x100000002 prazo=1234\r\n";
        assert_eq!(prazo_ciclo(linha, 0x100000002), Some(1234));
        assert_eq!(prazo_ciclo(linha, 0x200000002), None);
        assert_eq!(prazo_ciclo(&linha[..linha.len() - 1], 0x100000002), None);
        assert_eq!(
            prazo_ciclo(
                b"raiz> eco [teste-ciclo-espera] BLOQUEADA referencia=0x100000002 prazo=1234\n",
                0x100000002
            ),
            None
        );
    }
    #[test]
    fn t03_05_exige_empate_exato_um_timeout_uma_admissao_e_resultado_correto() {
        use super::validar_log_prazo_empate;
        let valido = "[teste-prazo-empate] KERNEL_FEATURE=1\n[teste-prazo-empate] ARMADA origem=0x100000001 destino=0x100000002 token=0x505\n[teste-prazo-empate] RECEBER_CHAMADA modo=1 duracao=40 tick=100 fila=0\n[teste-prazo-empate] ESPERA referencia=0x100000001 tick=100 prazo=140\n[teste-prazo-empate] SONDA_CHAMADA chamador=2 cpl=3 alvo=0x100000001 armada=0 registrada=1\n[teste-prazo-empate] SONDA_REGISTRADA referencia=0x100000001 prazo=140 timeout=0\n[teste-prazo-empate] TIMEOUT referencia=0x100000001 tick=140 prazo=140\n[teste-prazo-empate] ADMISSAO origem=0x100000002 destino=0x100000001 codigo=0x7f1 token=0x505 tick=140 prazo=140\n[teste-prazo-empate] RESULTADO primeiro=TEMPO segundo=IPC origem=0x100000002 codigo=0x7f1 prazo=140 token=0x505 CPL3\n";
        assert!(
            validar_log_prazo_empate(valido.as_bytes()).is_ok(),
            "fixture de referência rejeitada: {:?}",
            validar_log_prazo_empate(valido.as_bytes())
        );

        let outra_geracao_rede = valido.replace("0x100000002", "0x200000002");
        assert!(validar_log_prazo_empate(outra_geracao_rede.as_bytes()).is_err());

        let feature_ausente = valido.replace("[teste-prazo-empate] KERNEL_FEATURE=1\n", "");
        assert!(validar_log_prazo_empate(feature_ausente.as_bytes()).is_err());
        let sonda_sem_registro = valido.replace(
            "[teste-prazo-empate] SONDA_REGISTRADA referencia=0x100000001 prazo=140 timeout=0\n",
            "",
        );
        assert!(validar_log_prazo_empate(sonda_sem_registro.as_bytes()).is_err());

        let tick_divergente = valido.replace(
            "codigo=0x7f1 token=0x505 tick=140 prazo=140",
            "codigo=0x7f1 token=0x505 tick=141 prazo=140",
        );
        assert!(validar_log_prazo_empate(tick_divergente.as_bytes()).is_err());
        let admissao_antes_timeout = valido.replace(
            "[teste-prazo-empate] TIMEOUT referencia=0x100000001 tick=140 prazo=140\n[teste-prazo-empate] ADMISSAO",
            "[teste-prazo-empate] ADMISSAO",
        );
        let admissao_antes_timeout = admissao_antes_timeout.replace(
            "[teste-prazo-empate] ADMISSAO origem=0x100000002 destino=0x100000001 codigo=0x7f1 token=0x505 tick=140 prazo=140\n[teste-prazo-empate] RESULTADO",
            "[teste-prazo-empate] ADMISSAO origem=0x100000002 destino=0x100000001 codigo=0x7f1 token=0x505 tick=140 prazo=140\n[teste-prazo-empate] TIMEOUT referencia=0x100000001 tick=140 prazo=140\n[teste-prazo-empate] RESULTADO",
        );
        assert!(validar_log_prazo_empate(admissao_antes_timeout.as_bytes()).is_err());
        let duplicado = format!("{valido}[teste-prazo-empate] RESULTADO primeiro=TEMPO segundo=IPC origem=0x100000002 codigo=0x7f1 prazo=140 token=0x505 CPL3\n");
        assert!(validar_log_prazo_empate(duplicado.as_bytes()).is_err());
        let campo_duplicado = valido.replace(
            "[teste-prazo-empate] ESPERA referencia=0x100000001 tick=100 prazo=140",
            "[teste-prazo-empate] ESPERA referencia=0x100000001 tick=100 prazo=140 prazo=140",
        );
        assert!(validar_log_prazo_empate(campo_duplicado.as_bytes()).is_err());
        let campo_sufixo = valido.replace(
            "codigo=0x7f1 token=0x505 tick=140 prazo=140",
            "codigo=0x7f1 token=0x505 tick=140 prazo=140 extra=1",
        );
        assert!(validar_log_prazo_empate(campo_sufixo.as_bytes()).is_err());
        assert!(validar_log_prazo_empate(
            b"[teste-prazo-empate] ESPERA referencia=0x100000001 tick=100 prazo=140"
        )
        .is_err());
        assert!(validar_log_prazo_empate(
            b"raiz> eco [teste-prazo-empate] RESULTADO primeiro=TEMPO\n"
        )
        .is_err());
        for falha in [
            "[microkernel] FALHA_NUCLEO vetor=6",
            "[microkernel] SERVICO_FALHOU id=2 vetor=6",
            "[microkernel] SERVICO_FINALIZADO id=2 referencia=0x100000002",
            "[microkernel] PANICO teste",
            "thread panicked at fixture.rs:1",
        ] {
            let log_com_falha_depois = format!("{valido}{falha}\n");
            assert!(
                validar_log_prazo_empate(log_com_falha_depois.as_bytes()).is_err(),
                "falha após RESULTADO foi aceita: {falha}"
            );
        }
    }
    #[test]
    fn prazo_idle_exige_tres_bloqueios_antes_da_irq_e_timeout_antes_da_resposta() {
        use super::prazo_idle_ordenado;
        let valido = b"[teste-ipc] RECEBER_BLOQUEADO id=2\n[teste-prazo] BLOQUEADO id=3\n[teste-ipc] RECEBER_BLOQUEADO id=1\n[teste-prazo-idle] IRQ0_CPL0\n[teste-prazo] TIMEOUT id=3\n[teste-prazo-idle] RESPOSTA_CPL3\n";
        assert!(prazo_idle_ordenado(valido));
        assert!(!prazo_idle_ordenado(b"[teste-ipc] RECEBER_BLOQUEADO id=2\n[teste-prazo] BLOQUEADO id=3\n[teste-prazo-idle] IRQ0_CPL0\n[teste-ipc] RECEBER_BLOQUEADO id=1\n[teste-prazo] TIMEOUT id=3\n[teste-prazo-idle] RESPOSTA_CPL3\n"));
        assert!(!prazo_idle_ordenado(b"[teste-ipc] RECEBER_BLOQUEADO id=2\n[teste-prazo] BLOQUEADO id=3\n[teste-ipc] RECEBER_BLOQUEADO id=1\n[teste-prazo-idle] IRQ0_CPL0\n[teste-prazo-idle] RESPOSTA_CPL3\n[teste-prazo] TIMEOUT id=3\n"));
        assert!(!prazo_idle_ordenado(&valido[..valido.len() - 1]));
    }
    #[test]
    fn marcador_de_kernel_aceita_prompt_sem_aceitar_eco_ou_linha_parcial() {
        use super::{linha_corresponde, linhas_ordenadas};
        let marcador = "[teste-ipc] RECEBER_IMEDIATO id=3";
        assert!(linha_corresponde(
            "raiz> [teste-ipc] RECEBER_IMEDIATO id=3\r\n",
            marcador
        ));
        assert!(!linha_corresponde(
            "raiz> eco [teste-ipc] RECEBER_IMEDIATO id=3\n",
            marcador
        ));
        assert!(!linha_corresponde(
            "raiz> [teste-ipc] RECEBER_IMEDIATO id=3",
            marcador
        ));
        assert!(!linha_corresponde(
            "raiz> [teste-receber] RESPOSTA_FIFO_CPL3\n",
            "[teste-receber] RESPOSTA_FIFO_CPL3"
        ));
        assert!(linhas_ordenadas(
            b"raiz> [teste-ipc] RECEBER_IMEDIATO id=3\n[teste-ipc] RECEBER_BLOQUEADO id=3\n",
            marcador,
            "[teste-ipc] RECEBER_BLOQUEADO id=3"
        ));
        assert!(!linhas_ordenadas(
            b"[teste-ipc] RECEBER_BLOQUEADO id=3\nraiz> [teste-ipc] RECEBER_IMEDIATO id=3\n",
            marcador,
            "[teste-ipc] RECEBER_BLOQUEADO id=3"
        ));
    }
    #[test]
    fn reinicio_kernel_exige_marcador_completo_e_identidade_exata() {
        use super::{referencia_rede_geracao, reinicio_kernel_corresponde};
        let antiga = referencia_rede_geracao(2);
        let nova = referencia_rede_geracao(3);
        assert_eq!(antiga, 0x200000002);
        assert_eq!(nova, 0x300000002);
        let marcador = "[microkernel] SERVICO_REINICIADO id=2 antiga=0x200000002 nova=0x300000002 arena_antiga=0x600000 arena_nova=0x800000 purgadas=2\n";
        assert!(reinicio_kernel_corresponde(marcador, antiga, nova));
        assert!(reinicio_kernel_corresponde(
            &format!("raiz> {}\r\n", marcador.trim_end()),
            antiga,
            nova
        ));
        assert!(!reinicio_kernel_corresponde(
            "[microkernel] SERVICO_REINICIADO id=2 antiga=0x200000002 nova=0x300000002 arena_antiga=0x600000 arena_nova=0x800000 purgadas=2",
            antiga,
            nova
        ));
        assert!(!reinicio_kernel_corresponde(
            "[microkernel] SERVICO_REINICIADO id=2 antiga=0x200000002 nova=0x300000002 arena_antiga=0x600000 arena_nova=0x800000 purgadas=2 interrupcao=1\n",
            antiga,
            nova
        ));
        assert!(!reinicio_kernel_corresponde(
            "[supervisor] REDE_REINICIADA geracao_antiga=2 geracao_nova=3 automatico=0\n",
            antiga,
            nova
        ));
        assert!(!reinicio_kernel_corresponde(
            marcador,
            referencia_rede_geracao(1),
            nova
        ));
    }
    #[test]
    fn ipc_exige_despertar_antes_da_resposta_em_linhas_completas() {
        use super::marcadores_ipc_ordenados;
        assert!(marcadores_ipc_ordenados(
            b"[teste-ipc] DESPERTADO id=3\r\n[teste-ipc] RESPOSTA_FIFO_CPL3\r\n"
        ));
        assert!(!marcadores_ipc_ordenados(
            b"[teste-ipc] RESPOSTA_FIFO_CPL3\n[teste-ipc] DESPERTADO id=3\n"
        ));
        assert!(!marcadores_ipc_ordenados(
            b"eco [teste-ipc] DESPERTADO id=3\n[teste-ipc] RESPOSTA_FIFO_CPL3\n"
        ));
        assert!(!marcadores_ipc_ordenados(
            b"[teste-ipc] DESPERTADO id=3\n[teste-ipc] RESPOSTA_FIFO_CPL3"
        ));
    }
    use super::*;

    #[test]
    fn crc32_vetor_conhecido() {
        assert_eq!(crc32(b"123456789"), 0xcbf4_3926);
        assert_eq!(crc32(b""), 0);
    }

    #[test]
    fn log_de_arenas_rejeita_reuso_cadeia_quebrada_e_reboot() {
        let log = "[boot] EXIT_BOOT_SERVICES_OK\n\
            [microkernel] SERVICO_CARREGADO id=2 referencia=0x100000002 ELF=100 bytes fisico=0x400000 CR3=0x1000\n\
            [microkernel] SERVICO_REINICIADO id=2 antiga=0x100000002 nova=0x200000002 arena_antiga=0x400000 arena_nova=0x600000 purgadas=0\n\
            [microkernel] SERVICO_REINICIADO id=2 antiga=0x200000002 nova=0x300000002 arena_antiga=0x600000 arena_nova=0x800000 purgadas=2\n\
            [microkernel] SERVICO_REINICIADO id=2 antiga=0x300000002 nova=0x400000002 arena_antiga=0x800000 arena_nova=0xa00000 purgadas=0\n";
        assert_eq!(
            conferir_arenas_do_log(log.as_bytes(), 3).unwrap(),
            vec![0x400000, 0x600000, 0x800000, 0xa00000]
        );
        assert!(conferir_arenas_do_log(
            log.replace("arena_nova=0xa00000", "arena_nova=0x400000")
                .as_bytes(),
            3
        )
        .is_err());
        assert!(conferir_arenas_do_log(
            log.replace("arena_antiga=0x600000", "arena_antiga=0x400000")
                .as_bytes(),
            3
        )
        .is_err());
        assert!(conferir_arenas_do_log(
            format!("{log}[boot] EXIT_BOOT_SERVICES_OK\n").as_bytes(),
            3
        )
        .is_err());
        assert!(conferir_arenas_do_log(log.as_bytes(), 1).is_err());
    }

    #[test]
    fn estado_exige_linha_completa_sem_confundir_eco_do_comando() {
        assert_eq!(
            ultimo_estado_processos(b"raiz> processos\r\nterminal=1 rede=1 tiques=42\r\nraiz> "),
            Some((1, 1, 42))
        );
        assert_eq!(
            ultimo_estado_processos(b"terminal=1 rede=1 tiques=42"),
            None
        );
        assert_eq!(
            ultimo_estado_processos(b"raiz> eco terminal=1 rede=1 tiques=42\r\n"),
            None
        );
        assert_eq!(
            ultimo_estado_processos(
                b"terminal=1 rede=1 tiques=42\r\nterminal=1 rede=2 tiques=53\r\n"
            ),
            Some((1, 2, 53))
        );
    }

    #[test]
    fn imagem_preserva_arquivo_e_detecta_corrupcao_gpt() -> Resultado<()> {
        let instante = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_nanos();
        let caminho =
            env::temp_dir().join(format!("raiz-imagem-{}-{instante}.img", std::process::id()));
        let efi: Vec<u8> = (0..2033).map(|i| (i % 251) as u8).collect();
        criar_imagem(&caminho, &efi)?;
        verificar_imagem(&caminho, &efi)?;
        let mut arquivo = fs::OpenOptions::new().write(true).open(&caminho)?;
        arquivo.seek(SeekFrom::Start(SETOR as u64 + 40))?;
        arquivo.write_all(&[0xff])?;
        drop(arquivo);
        assert!(verificar_imagem(&caminho, &efi).is_err());
        fs::remove_file(caminho)?;
        Ok(())
    }

    #[test]
    fn oraculo_t03_16_correlaciona_host_e_rbr_por_fase_e_trata_oe_como_flag() {
        use super::validar_log_uart_rz025;
        let mut log = String::from(
            "[teste-uart-rz025] CONFIG FCR=0xc7 RX_LIMIAR=14\n\
             [teste-uart-rz025] FASE_PRONTA fase=UNICO\n\
             [teste-uart-rz025] IRQ4 fase=UNICO seq=11 rbr_antes=0 iir_raw=0xcc iir_pending_bit=0 causa=0x0c\n\
             [teste-uart-rz025] RBR fase=UNICO id=1 valor=0x55\n\
             [teste-uart-rz025] FASE_FIM fase=UNICO rbr=1 lsr_oe=0 irq4=1 iir_rda=0 iir_timeout=1\n\
             [teste-uart-rz025] FASE_PRONTA fase=RAJADA\n\
             [teste-uart-rz025] IRQ4 fase=RAJADA seq=12 rbr_antes=0 iir_raw=0xc4 iir_pending_bit=0 causa=0x04\n",
        );
        for (indice, valor) in (0x31u8..=0x38).enumerate() {
            log.push_str(&format!(
                "[teste-uart-rz025] RBR fase=RAJADA id={} valor={:#04x}\n",
                indice + 1,
                valor
            ));
        }
        log.push_str(
            "[teste-uart-rz025] FASE_FIM fase=RAJADA rbr=8 lsr_oe=1 irq4=1 iir_rda=1 iir_timeout=0\n\
             [teste-uart-rz025] FIM CPL3\n",
        );
        assert!(validar_log_uart_rz025(log.as_bytes()).is_ok());
        assert!(validar_log_uart_rz025(
            log.replace("id=8 valor=0x38", "id=8 valor=0x37").as_bytes()
        )
        .is_err());
        assert!(validar_log_uart_rz025(
            log.replace("id=8 valor=0x38", "id=7 valor=0x38").as_bytes()
        )
        .is_err());
        assert!(validar_log_uart_rz025(log.replace("lsr_oe=1", "lsr_oe=10").as_bytes()).is_err());
        assert!(validar_log_uart_rz025(log.replace("id=4 valor=0x34\n", "").as_bytes()).is_err());
        assert!(
            validar_log_uart_rz025(log.replace("FASE_PRONTA fase=RAJADA\n", "").as_bytes())
                .is_err()
        );
        assert!(validar_log_uart_rz025(log.replace("FIM CPL3\n", "FIM CPL3").as_bytes()).is_err());
        assert!(validar_log_uart_rz025(
            log.replace(
                "[teste-uart-rz025] IRQ4 fase=RAJADA seq=12 rbr_antes=0 iir_raw=0xc4 iir_pending_bit=0 causa=0x04\n",
                "[teste-uart-rz025] IRQ4 fase=RAJADA seq=12 rbr_antes=0 iir_raw=0xc5 iir_pending_bit=1 causa=0x04\n"
            )
            .as_bytes()
        )
        .is_err());
        assert!(validar_log_uart_rz025(
            log.replace(
                "iir_raw=0xc4 iir_pending_bit=0 causa=0x04",
                "iir_raw=0xc5 iir_pending_bit=0 causa=0x04"
            )
            .as_bytes()
        )
        .is_err());
        assert!(validar_log_uart_rz025(
            log.replace(
                "IRQ4 fase=RAJADA seq=12 rbr_antes=0 iir_raw=0xc4 iir_pending_bit=0 causa=0x04\n",
                ""
            )
            .as_bytes()
        )
        .is_err());
        assert!(validar_log_uart_rz025(
            log.replace(
                "iir_raw=0xc4 iir_pending_bit=0 causa=0x04",
                "iir_raw=0xc6 iir_pending_bit=0 causa=0x06"
            )
            .as_bytes()
        )
        .is_err());
        assert!(validar_log_uart_rz025(
            log.replace(
                "iir_raw=0xc4 iir_pending_bit=0 causa=0x04",
                "iir_raw=0xc4 iir_pending_bit=0 causa=0x0c"
            )
            .as_bytes()
        )
        .is_err());
        assert!(validar_log_uart_rz025(
            log.replace("rbr_antes=0 iir_raw=0xc4", "rbr_antes=1 iir_raw=0xc4")
                .as_bytes()
        )
        .is_err());
        assert!(validar_log_uart_rz025(log.replace("seq=12", "seq=10").as_bytes()).is_err());
        assert!(validar_log_uart_rz025(
            log.replace(
                "[teste-uart-rz025] IRQ4 fase=UNICO seq=11 rbr_antes=0 iir_raw=0xcc iir_pending_bit=0 causa=0x0c\n[teste-uart-rz025] RBR fase=UNICO id=1 valor=0x55\n",
                "[teste-uart-rz025] RBR fase=UNICO id=1 valor=0x55\n[teste-uart-rz025] IRQ4 fase=UNICO seq=11 rbr_antes=0 iir_raw=0xcc iir_pending_bit=0 causa=0x0c\n"
            )
            .as_bytes()
        )
        .is_err());
        assert!(validar_log_uart_rz025(log.replace("seq=12", "seq=11").as_bytes()).is_err());
        let segunda_amostra = "[teste-uart-rz025] IRQ4 fase=RAJADA seq=13 rbr_antes=4 iir_raw=0xc4 iir_pending_bit=0 causa=0x04\n";
        let duas_amostras = log
            .replace(
                "[teste-uart-rz025] RBR fase=RAJADA id=4 valor=0x34\n",
                &format!("[teste-uart-rz025] RBR fase=RAJADA id=4 valor=0x34\n{segunda_amostra}"),
            )
            .replace(
                "rbr=8 lsr_oe=1 irq4=1 iir_rda=1 iir_timeout=0",
                "rbr=8 lsr_oe=1 irq4=2 iir_rda=2 iir_timeout=0",
            );
        assert!(validar_log_uart_rz025(duas_amostras.as_bytes()).is_ok());
        assert!(validar_log_uart_rz025(
            duas_amostras
                .replace(segunda_amostra, "")
                .replace(
                    "[teste-uart-rz025] IRQ4 fase=RAJADA seq=12 rbr_antes=0 iir_raw=0xc4 iir_pending_bit=0 causa=0x04\n",
                    &format!("[teste-uart-rz025] IRQ4 fase=RAJADA seq=12 rbr_antes=0 iir_raw=0xc4 iir_pending_bit=0 causa=0x04\n{segunda_amostra}"),
                )
                .as_bytes()
        )
        .is_err());
        assert!(super::validar_estimulo_host_rz025(
            b"fase=UNICO submitted=55 quantidade=1\nfase=RAJADA submitted=31,32,33,34,35,36,37,38 quantidade=8\n"
        )
        .is_ok());
        assert!(super::validar_estimulo_host_rz025(
            b"fase=UNICO submitted=55 quantidade=1\nfase=RAJADA submitted=31,32,33,34,37,36,37,38 quantidade=8\n"
        )
        .is_err());
    }

    #[test]
    fn prazo_fixture_ciclo_ancora_no_driver_e_ignora_esperas_de_inicializacao() {
        use super::{prazo_ciclo, prazo_ciclo_apos_driver, timeouts_esperas_fixture_ciclo};
        let serial = concat!(
            "[teste-ciclo-espera] BLOQUEADA referencia=0x200000002 prazo=133\n",
            "[teste-ciclo-espera] TIMEOUT referencia=0x200000002 tiques=133\n",
            "[teste-ciclo-espera] BLOQUEADA referencia=0x200000002 prazo=135\n",
            "[teste-ciclo-espera] TIMEOUT referencia=0x200000002 tiques=135\n",
            "[teste-ciclo-espera] DRIVER_ATIVO referencia=0x200000002 mmio=0x81040000 arena=0x26b9000 rx_idx=0 tx_idx=0 rx=0 tx=0 CPL3\n",
            "[teste-ciclo-espera] BLOQUEADA referencia=0x200000002 prazo=5135\n",
            "[teste-ciclo-espera] TIMEOUT referencia=0x200000002 tiques=5135\n",
        );
        assert_eq!(prazo_ciclo(serial.as_bytes(), 0x200000002), Some(133));
        assert_eq!(
            prazo_ciclo_apos_driver(serial.as_bytes(), 0x200000002),
            Some(5135)
        );
        assert_eq!(
            timeouts_esperas_fixture_ciclo(serial.as_bytes()),
            vec![(0x200000002, 5135)]
        );
        assert_eq!(
            prazo_ciclo_apos_driver(serial.as_bytes(), 0x300000002),
            None
        );
    }

    #[test]
    fn prontidao_rede_precisa_preceder_snapshot_do_driver_da_mesma_geracao() {
        use super::validar_pronto_antes_driver_ciclo;
        let pronto_anterior = "[rede] SERVICO_PRONTO CPL=3 IPv4=10.0.2.15 UDP=7777 geracao=1\n";
        let pronto = "[rede] SERVICO_PRONTO CPL=3 IPv4=10.0.2.15 UDP=7777 geracao=2\n";
        let driver = "[teste-ciclo-espera] DRIVER_ATIVO referencia=0x200000002 mmio=0x50000000 arena=0x26b9000 rx_idx=0 tx_idx=0 rx=0 tx=0 CPL3\n";
        assert!(validar_pronto_antes_driver_ciclo(
            format!("{pronto_anterior}{pronto}{driver}").as_bytes(),
            0x200000002
        )
        .is_ok());
        assert!(validar_pronto_antes_driver_ciclo(
            format!("{driver}{pronto}").as_bytes(),
            0x200000002
        )
        .is_err());
        assert!(validar_pronto_antes_driver_ciclo(
            format!("{pronto}{driver}").as_bytes(),
            0x300000002
        )
        .is_err());
        assert!(validar_pronto_antes_driver_ciclo(
            format!("{pronto_anterior}{pronto}{pronto}{driver}").as_bytes(),
            0x200000002
        )
        .is_err());
        assert!(validar_pronto_antes_driver_ciclo(
            format!("{pronto_anterior}[rede] SERVICO_PRONTO CPL=3 IPv4=10.0.2.15 UDP=7777 geracao=2x\n{driver}")
                .as_bytes(),
            0x200000002
        )
        .is_err());
    }

    #[test]
    fn oraculo_comparacao_vazia_exige_marcadores_unicos_e_delta_exato() {
        let valido = concat!(
            "[teste] TERMINAL_PRONTO\n",
            "[rede] SERVICO_PRONTO CPL=3 geracao=1\n",
            "[supervisor] SERVICO_PRONTO CPL=3\n",
            "[comparacao-vazia] ARMADA revisao=RZ-025 janela_tiques=100 inicio=500\n",
            "[comparacao-vazia] RESULTADO revisao=RZ-025 janela_tiques=100 inicio=500 fim=600 decorrido=100 consultas_ipc_vazias=123 voltas_sem_trabalho=456\n",
        );
        let medicao = validar_log_comparacao_vazia(valido.as_bytes()).unwrap();
        assert_eq!(medicao.inicio, 500);
        assert_eq!(medicao.fim, 600);
        assert_eq!(medicao.consultas_ipc_vazias, 123);
        assert_eq!(medicao.voltas_sem_trabalho, 456);

        assert!(validar_log_comparacao_vazia(
            format!(
                "{valido}[comparacao-vazia] ARMADA revisao=RZ-025 janela_tiques=100 inicio=500\n"
            )
            .as_bytes()
        )
        .is_err());
        assert!(validar_log_comparacao_vazia(
            format!("{valido}[comparacao-vazia] RESULTADO revisao=RZ-025 janela_tiques=100 inicio=500 fim=600 decorrido=100 consultas_ipc_vazias=123 voltas_sem_trabalho=456\n").as_bytes()
        )
        .is_err());
        assert!(validar_log_comparacao_vazia(
            valido.replace("decorrido=100", "decorrido=101").as_bytes()
        )
        .is_err());
        assert!(
            validar_log_comparacao_vazia(valido.replace("fim=600", "fim=601").as_bytes()).is_err()
        );
        assert!(validar_log_comparacao_vazia(
            valido.replace("revisao=RZ-025", "revisao=0.2").as_bytes()
        )
        .is_err());
        assert!(validar_log_comparacao_vazia(
            valido
                .replace(
                    "consultas_ipc_vazias=123",
                    "consultas_ipc_vazias=123 consultas_ipc_vazias=124"
                )
                .as_bytes()
        )
        .is_err());
        assert!(validar_log_comparacao_vazia(
            valido
                .replace(
                    "[comparacao-vazia] ARMADA",
                    "raiz> [comparacao-vazia] ARMADA"
                )
                .as_bytes()
        )
        .is_err());
        assert!(validar_log_comparacao_vazia(
            valido
                .replace(
                    "[comparacao-vazia] RESULTADO",
                    "[comparacao-vazia] FALHA_JANELA"
                )
                .as_bytes()
        )
        .is_err());
        assert!(validar_log_comparacao_vazia(
            b"[comparacao-vazia] ARMADA revisao=RZ-025 janela_tiques=100 inicio=500\n[comparacao-vazia] RESULTADO revisao=RZ-025"
        )
        .is_err());
        assert!(validar_log_comparacao_vazia(
            valido
                .replace(
                    "[comparacao-vazia] RESULTADO",
                    "[comparacao-vazia] RESULTADO_extra"
                )
                .as_bytes()
        )
        .is_err());
    }

    #[test]
    fn rejeita_arquivo_vazio() {
        let caminho = env::temp_dir().join("raiz-nao-criar-imagem-vazia.img");
        assert!(criar_imagem(&caminho, &[]).is_err());
    }
}
