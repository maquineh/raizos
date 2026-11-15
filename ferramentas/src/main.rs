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
        "compilar" if argumentos.len() == 1 => compilar(&raiz),
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
            println!("Raiz OS — ferramentas em Rust\n\n  cargo run -p ferramentas -- compilar\n  cargo run -p ferramentas -- verificar-imagem\n  cargo run -p ferramentas -- executar [--grafico]\n  cargo run -p ferramentas -- testar\n  cargo run -p ferramentas -- testar-isolamento\n  cargo run -p ferramentas -- testar-recuperacao\n  cargo run -p ferramentas -- eco-udp TEXTO\n\nVariáveis opcionais:\n  RAIZ_QEMU        executável qemu-system-x86_64\n  RAIZ_QEMU_DADOS  diretório de ROMs/dados do QEMU (-L)\n  RAIZ_OVMF        arquivo OVMF_CODE.fd / OVMF_CODE_4M.fd\n  RAIZ_OVMF_VARS   arquivo OVMF_VARS.fd correspondente\n\nA imagem gerada é saida/raiz-os.img (128 MiB). Ctrl+C encerra o emulador.");
            Ok(())
        }
        _ => Err("Comando inválido. Use: compilar, executar [--grafico], testar, testar-isolamento, testar-recuperacao, eco-udp TEXTO, verificar-imagem ou ajuda".into()),
    }
}

fn compilar(raiz: &Path) -> Resultado<()> {
    let cargo = env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let destino = raiz.join("target");
    println!("Compilando serviços de terminal, rede e supervisão...");
    let estado = Command::new(&cargo)
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
        .arg(&destino)
        .status()?;
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
    let estado = Command::new(&cargo)
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
        .env("RAIZ_SUPERVISOR", &supervisor)
        .status()?;
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
    let esp = raiz.join("saida/esp/EFI/BOOT");
    fs::create_dir_all(&esp)?;
    fs::write(esp.join("BOOTX64.EFI"), &efi)?;
    let imagem = raiz.join("saida/raiz-os.img");
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
    let imagem = raiz.join("saida/raiz-os.img");
    if !imagem.is_file() {
        compilar(raiz)?;
    }
    let efi = fs::read(raiz.join("saida/esp/EFI/BOOT/BOOTX64.EFI"))?;
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
            "user,id=rede,hostfwd=udp:127.0.0.1:17777-10.0.2.15:7777",
            "-serial",
            "stdio",
            "-monitor",
            "none",
            "-no-reboot",
        ]);
    if let Some(dados) = env::var_os("RAIZ_QEMU_DADOS") {
        comando.arg("-L").arg(dados);
    }
    if !grafico {
        comando.args(["-display", "none"]);
    }
    if let Some(variaveis) = localizar_variaveis(&firmware) {
        let copia = raiz.join(if teste {
            "saida/OVMF_VARS_TESTE.fd"
        } else {
            "saida/OVMF_VARS.fd"
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
                .any(|l| l.ends_with('\n') && l.trim_end_matches(['\r', '\n']) == linha)
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
                    return Err("Os fluxos do emulador foram encerrados".into())
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
    let mut relatorio = format!("# Teste de integração do Raiz OS\n\nResultado: **{}**.\n\nDuração: {:.3} segundos.\n\nAmbiente solicitado: QEMU/TCG, pc, 1 CPU, 256 MiB, OVMF, e1000.\n\n## Etapas concluídas\n\n",
        if resultado.is_ok() { "APROVADO" } else { "FALHOU" }, inicio.elapsed().as_secs_f64());
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
        entrada.write_all(b"sobre\nmemoria\nping 10.0.2.2\n")?;
        entrada.flush()?;
        captura.aguardar(&mut processo, "CPL=3", inicio_sobre, Duration::from_secs(5))?;
        captura.aguardar(
            &mut processo,
            "PING OK 10.0.2.2",
            0,
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
        entrada.write_all(b"rede\n")?;
        entrada.flush()?;
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
        entrada.write_all(b"teste-cap\n")?;
        entrada.flush()?;
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
        entrada.write_all(b"falhar-rede\n")?;
        entrada.flush()?;
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
        entrada.write_all(b"eco TERMINAL_VIVO_APOS_FALHA\nprocessos\nping 10.0.2.2\n")?;
        entrada.flush()?;
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
    let mut relatorio = format!("# Testes de memória e preempção do Raiz OS\n\nResultado geral: **{}**.\n\nDuração: {:.3} segundos.\n\nAmbiente solicitado: duas inicializações independentes e sequenciais em QEMU/TCG, pc, 1 CPU, 256 MiB, OVMF e e1000.\n\n",
        if aprovado { "APROVADO" } else { "FALHOU" }, inicio.elapsed().as_secs_f64());
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
                entrada.write_all(b"violar-memoria\n")?;
                entrada.flush()?;
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
                entrada.write_all(b"eco MEMORIA_PROTEGIDA_TERMINAL_VIVO\n")?;
                entrada.flush()?;
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
                entrada.write_all(b"ocupar-rede\n")?;
                entrada.flush()?;
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
                entrada.write_all(b"eco PREEMPCAO_OK\n")?;
                entrada.flush()?;
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
    entrada.write_all(b"processos\n")?;
    entrada.flush()?;
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
    let mut relatorio = format!("# Recuperação de serviços — Raiz OS 0.2\n\nResultado geral: **{}**.\n\nDuração: {:.3} segundos.\n\nAmbiente solicitado: QEMU/TCG, pc, 1 CPU, 256 MiB, OVMF e e1000. Dois boots independentes e sequenciais.\n\n",
        if aprovado { "APROVADO" } else { "FALHOU" }, inicio.elapsed().as_secs_f64());
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

fn enviar_comando(
    entrada: &mut std::process::ChildStdin,
    captura: &mut Captura,
    comando: &str,
) -> Resultado<usize> {
    captura.drenar()?;
    let inicio = captura.serial.len();
    entrada.write_all(comando.as_bytes())?;
    entrada.flush()?;
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
        |bytes| {
            String::from_utf8_lossy(bytes)
                .split_inclusive('\n')
                .any(|linha| {
                    linha.ends_with('\n')
                        && linha.contains(marcador)
                        && campos.iter().all(|esperado| {
                            linha.split_whitespace().any(|campo| campo == *esperado)
                        })
                })
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
    fn rejeita_arquivo_vazio() {
        let caminho = env::temp_dir().join("raiz-nao-criar-imagem-vazia.img");
        assert!(criar_imagem(&caminho, &[]).is_err());
    }
}
