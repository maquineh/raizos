#![no_std]
#![no_main]

mod console;
mod fonte;
mod io;
mod teclado;

use apoio_servicos as sistema;
use contrato_sistema::{
    chamada, erro, evento, geracao_referencia, id_referencia, referencia_valida, InfoServico,
    Mensagem, ASSINATURA_SERVICO, REDE, SUPERVISOR, TERMINAL, VERSAO_ABI,
};

struct ContextoTerminal {
    referencia_guardada: Option<u64>,
}

#[no_mangle]
#[link_section = ".text.entrada"]
pub unsafe extern "sysv64" fn _inicio_servico(informacoes: *const InfoServico) -> ! {
    let info = *informacoes;
    if info.assinatura != ASSINATURA_SERVICO
        || info.versao != VERSAO_ABI
        || info.id != TERMINAL
        || !referencia_valida(info.referencia)
        || id_referencia(info.referencia) != TERMINAL
    {
        sistema::sair();
    }
    io::iniciar_serial();
    console::iniciar(info.tela);
    imprimirln!(
        "RAIZ OS / {} / MICROKERNEL / x86_64",
        env!("CARGO_PKG_VERSION")
    );
    imprimirln!("Sistema operacional proprio, escrito em Rust.");
    imprimirln!(
        "[terminal] Servico isolado iniciado: CPL={}",
        sistema::cpl()
    );
    imprimirln!("[ok] Console, framebuffer e teclado fora do kernel.");
    imprimirln!("[ok] IPC int80; paginas por processo; privilegios por servico.");
    let mut teclado = teclado::Teclado::iniciar();
    imprimirln!("Digite ajuda. Use ping 10.0.2.2 para testar a rede.");
    imprimirln!("[teste] TERMINAL_PRONTO");
    imprimir!("raiz> ");
    let mut linha = [0u8; 256];
    let mut tamanho = 0;
    let mut ultimo_cr = false;
    let mut contexto = ContextoTerminal {
        referencia_guardada: None,
    };
    loop {
        for _ in 0..16 {
            let Some(m) = sistema::receber() else { break };
            if m.origem == sistema::resolver(SUPERVISOR) {
                receber_supervisor(&m);
                continue;
            }
            if m.origem != sistema::resolver(REDE) {
                continue;
            }
            match m.codigo {
                evento::REDE_PRONTA => {
                    if m.a == 0 {
                        imprimirln!(
                            "\n[rede] SERVICO_PRONTO CPL={} IPv4=10.0.2.15 UDP=7777 geracao={}",
                            m.d,
                            geracao_referencia(m.origem)
                        );
                    } else {
                        imprimirln!("\n[rede] Falha ao inicializar driver e1000.");
                    }
                    imprimir!("raiz> ");
                }
                evento::PING_OK => {
                    let ip = (m.a as u32).to_be_bytes();
                    imprimirln!(
                        "\n[rede] PING OK {}.{}.{}.{} seq={}",
                        ip[0],
                        ip[1],
                        ip[2],
                        ip[3],
                        m.b
                    );
                    imprimir!("raiz> ");
                }
                evento::REDE_STATUS => {
                    imprimirln!("\nIPv4=10.0.2.15/24 gateway=10.0.2.2 / servico ring 3");
                    imprimirln!("RX={} TX={} ping_ok={} udp_eco={}", m.a, m.b, m.c, m.d);
                    imprimir!("raiz> ");
                }
                evento::REDE_ERRO => {
                    imprimirln!("\n[rede] Operacao recusada: codigo={}", m.a);
                    imprimir!("raiz> ");
                }
                evento::REDE_EM_LACO => {
                    imprimirln!("\n[rede] LOOP_SEM_CEDER");
                    imprimir!("raiz> ");
                }
                _ => {}
            }
        }
        if let Some(b) = io::ler_serial().or_else(|| teclado.ler()) {
            if b == b'\n' && ultimo_cr {
                ultimo_cr = false;
                continue;
            }
            ultimo_cr = b == b'\r';
            match b {
                b'\r' | b'\n' => {
                    imprimirln!();
                    let comando = core::str::from_utf8(&linha[..tamanho]).unwrap_or("").trim();
                    executar(comando, &info, &mut contexto);
                    tamanho = 0;
                    imprimir!("raiz> ");
                }
                8 | 127 => {
                    if tamanho > 0 {
                        tamanho -= 1;
                        imprimir!("\x08 \x08");
                    }
                }
                3 => {
                    tamanho = 0;
                    imprimirln!("^C");
                    imprimir!("raiz> ");
                }
                12 => {
                    console::limpar();
                    tamanho = 0;
                    imprimir!("raiz> ");
                }
                32..=126 => {
                    if tamanho < linha.len() {
                        linha[tamanho] = b;
                        tamanho += 1;
                        imprimir!("{}", b as char);
                    }
                }
                _ => {}
            }
        }
        sistema::ceder();
    }
}

fn receber_supervisor(m: &Mensagem) {
    match m.codigo {
        evento::SUPERVISOR_PRONTO => {
            imprimirln!(
                "\n[supervisor] SERVICO_PRONTO CPL={} modo=manual limite={}",
                m.c,
                m.b
            );
        }
        evento::SUPERVISAO_ESTADO => {
            let modo = if m.a == 0 { "manual" } else { "auto" };
            imprimirln!(
                "\n[supervisor] MODO {} reinicios={} ultimo_erro={:#x} limite={}",
                modo,
                m.b,
                m.c,
                m.d
            );
        }
        evento::SERVICO_REINICIADO => {
            imprimirln!("\n[supervisor] REDE_REINICIADA geracao_antiga={} geracao_nova={} automatico={} referencia_antiga={:#x} referencia_nova={:#x}",
                geracao_referencia(m.a), geracao_referencia(m.b), m.c, m.a, m.b);
            imprimirln!("Nova instancia criada; aguardando inicializacao do driver.");
        }
        evento::SERVICO_ENCERRADO => {
            imprimirln!(
                "\n[supervisor] REDE_PARADA geracao={}",
                geracao_referencia(m.a)
            );
        }
        evento::SUPERVISOR_ERRO => {
            if m.a == erro::RECURSOS {
                imprimirln!(
                    "\n[supervisor] RECURSOS_ESGOTADOS operacao={} geracao={}",
                    m.b,
                    geracao_referencia(m.c)
                );
                imprimirln!("Limite de arenas/recuperacoes do boot atingido; consulte processos.");
            } else {
                imprimirln!(
                    "\n[supervisor] OPERACAO_RECUSADA codigo={:#x} operacao={} geracao={}",
                    m.a,
                    m.b,
                    geracao_referencia(m.c)
                );
            }
        }
        _ => return,
    }
    imprimir!("raiz> ");
}

fn enviar_rede(codigo: u64, a: u64) {
    let destino = sistema::resolver(REDE);
    match sistema::enviar(destino, codigo, a, 0, 0, 0) {
        0 => {}
        erro::SERVICO => {
            imprimirln!("Servico de rede indisponivel; terminal continua ativo.");
            imprimirln!("[teste] IPC_SERVICO_INDISPONIVEL_OK");
        }
        erro::REFERENCIA_ANTIGA => {
            imprimirln!("A rede mudou de geracao durante o pedido. Repita o comando.")
        }
        erro::FILA_CHEIA => imprimirln!("Fila IPC cheia. Tente novamente."),
        e => imprimirln!("IPC recusado: {:#x}", e),
    }
}

fn enviar_supervisor(codigo: u64, argumento: u64) {
    let destino = sistema::resolver(SUPERVISOR);
    match sistema::enviar(destino, codigo, argumento, 0, 0, 0) {
        0 => {}
        erro::SERVICO => imprimirln!("Supervisor indisponivel; a operacao nao foi executada."),
        erro::FILA_CHEIA => imprimirln!("Fila do supervisor cheia. Tente novamente."),
        e => imprimirln!("Pedido ao supervisor recusado: {:#x}", e),
    }
}

fn executar(comando: &str, info: &InfoServico, contexto: &mut ContextoTerminal) {
    let mut partes = comando.split_whitespace();
    match partes.next().unwrap_or("") {
        "" => {}
        "ajuda" => {
            imprimirln!("sobre          - arquitetura e CPL do terminal");
            imprimirln!("memoria        - reservas e isolamento de enderecos");
            imprimirln!("processos      - estados dos servicos e tiques do timer");
            imprimirln!("rede           - pede estatisticas ao servico via IPC");
            imprimirln!("ping [IPv4]    - solicita ICMP ao servico de rede");
            imprimirln!("eco TEXTO      - imprime no console");
            imprimirln!("limpar         - limpa console e framebuffer");
            imprimirln!("teste-cap      - testa negacao de acesso PCI ao terminal");
            imprimirln!("teste-supervisor - terminal tenta reiniciar/parar sem direito");
            imprimirln!("falhar-rede    - provoca UD2 na rede; testa isolamento");
            imprimirln!("violar-memoria - rede tenta ler o nucleo; testa a MMU");
            imprimirln!("ocupar-rede    - loop infinito na rede; testa preempcao");
            imprimirln!("reiniciar-rede - supervisor cria nova instancia de rede");
            imprimirln!("parar-rede     - supervisor para a rede e muda para manual");
            imprimirln!("supervisao [auto|manual] - consulta ou altera a politica");
            imprimirln!("guardar-referencia - conserva a identidade atual da rede");
            imprimirln!("teste-referencia - testa envio e estado da identidade salva");
            imprimirln!("Limite: 4 instancias de rede por boot; arenas antigas em quarentena.");
        }
        "sobre" => {
            imprimirln!(
                "Raiz OS {} - microkernel proprio em Rust.",
                env!("CARGO_PKG_VERSION")
            );
            imprimirln!(
                "Terminal PID={} CPL={} / rede PID=2 / supervisor PID=3 / nucleo CPL=0",
                info.id,
                sistema::cpl()
            );
            imprimirln!("Nucleo: memoria, processos, IPC, interrupcoes e permissoes fixas.");
            imprimirln!("Servicos: terminal+console; rede+e1000; supervisor de recuperacao.");
            imprimirln!("Referencias incluem geracao; direitos sao verificados separadamente.");
            imprimirln!("Ainda sem arquivos persistentes, TCP ou IOMMU; driver DMA confiavel.");
        }
        "memoria" => {
            imprimirln!(
                "Memoria convencional recebida: {} MiB.",
                info.memoria_livre_boot / 1024 / 1024
            );
            imprimirln!(
                "Imagem deste processo: VA 0x40000000, segmentos {} KiB.",
                info.tamanho_imagem / 1024
            );
            imprimirln!("Reserva fisica: 2 MiB; paginas atribuidas conforme segmentos/pilha.");
            imprimirln!("Pilha propria; texto RX; dados RW+NX; nucleo supervisor.");
            imprimirln!("Tres servicos usam o mesmo VA com memoria fisica distinta.");
            imprimirln!("Reinicios da rede recebem arena nova; as antigas nao sao reutilizadas.");
        }
        "processos" => {
            imprimirln!(
                "terminal={} rede={} tiques={}",
                sistema::estado(TERMINAL),
                sistema::estado(REDE),
                sistema::tiques()
            );
            let referencia_rede = sistema::resolver(REDE);
            imprimirln!(
                "supervisor={} geracao_rede={} referencia_rede={:#x} referencia_terminal={:#x}",
                sistema::estado(SUPERVISOR),
                geracao_referencia(referencia_rede),
                referencia_rede,
                info.referencia
            );
            imprimirln!("Estados: 1=executavel 2=falhou 3=finalizado 0=inexistente.");
        }
        "limpar" => console::limpar(),
        "eco" => imprimirln!("{}", comando.strip_prefix("eco").unwrap_or("").trim_start()),
        "rede" => enviar_rede(evento::REDE_STATUS_SOLICITAR, 0),
        "ping" => {
            let endereco = partes.next().unwrap_or("10.0.2.2");
            if let Some(ip) = ler_ip(endereco) {
                imprimirln!("Solicitando ping a {} por IPC...", endereco);
                enviar_rede(evento::PING_SOLICITAR, u32::from_be_bytes(ip) as u64);
            } else {
                imprimirln!("IPv4 invalido. Exemplo: ping 10.0.2.2");
            }
        }
        "teste-cap" => {
            let resultado = sistema::chamar(chamada::PCI_LER, 0, 0, 0, 0, 0, 0);
            if resultado == erro::PERMISSAO {
                imprimirln!("[teste] CAPACIDADE_NEGADA_OK");
            } else {
                imprimirln!("[teste] ERRO_CAPACIDADE retorno={:#x}", resultado);
            }
        }
        "teste-supervisor" => {
            let referencia = sistema::resolver(REDE);
            let reinicio = sistema::reiniciar(referencia);
            let encerramento = sistema::encerrar(referencia);
            if reinicio == erro::PERMISSAO {
                imprimirln!("[teste] SUPERVISOR_REINICIAR_NEGADO_OK");
            } else {
                imprimirln!("[teste] ERRO_SUPERVISOR_REINICIAR retorno={:#x}", reinicio);
            }
            if encerramento == erro::PERMISSAO {
                imprimirln!("[teste] SUPERVISOR_ENCERRAR_NEGADO_OK");
            } else {
                imprimirln!(
                    "[teste] ERRO_SUPERVISOR_ENCERRAR retorno={:#x}",
                    encerramento
                );
            }
        }
        "reiniciar-rede" => {
            enviar_supervisor(evento::SERVICO_REINICIAR, sistema::resolver(REDE));
        }
        "parar-rede" => {
            enviar_supervisor(evento::SERVICO_ENCERRAR, sistema::resolver(REDE));
        }
        "supervisao" => match partes.next() {
            None => enviar_supervisor(evento::SUPERVISAO_CONSULTAR, 0),
            Some("auto") => enviar_supervisor(evento::SUPERVISAO_CONFIGURAR, 1),
            Some("manual") => enviar_supervisor(evento::SUPERVISAO_CONFIGURAR, 0),
            _ => imprimirln!("Use: supervisao [auto|manual]"),
        },
        "guardar-referencia" => {
            let referencia = sistema::resolver(REDE);
            if referencia_valida(referencia) {
                contexto.referencia_guardada = Some(referencia);
                imprimirln!(
                    "[teste] REFERENCIA_GUARDADA geracao={} referencia={:#x}",
                    geracao_referencia(referencia),
                    referencia
                );
            } else {
                imprimirln!("Nao foi possivel resolver a referencia da rede.");
            }
        }
        "teste-referencia" => {
            if let Some(guardada) = contexto.referencia_guardada {
                // Não resolver novamente: este teste precisa apresentar ao kernel
                // exatamente a identidade conservada antes do reinício.
                let envio = sistema::enviar(guardada, evento::REDE_STATUS_SOLICITAR, 0, 0, 0, 0);
                let estado = sistema::estado_referencia(guardada);
                if envio == erro::REFERENCIA_ANTIGA {
                    imprimirln!("[teste] REFERENCIA_ANTIGA_ENVIO_OK");
                } else {
                    imprimirln!("[teste] ERRO_REFERENCIA_ANTIGA_ENVIO retorno={:#x}", envio);
                }
                if estado == erro::REFERENCIA_ANTIGA {
                    imprimirln!("[teste] REFERENCIA_ANTIGA_ESTADO_OK");
                } else {
                    imprimirln!(
                        "[teste] ERRO_REFERENCIA_ANTIGA_ESTADO retorno={:#x}",
                        estado
                    );
                }
            } else {
                imprimirln!("Use guardar-referencia, reinicie a rede e depois teste-referencia.");
            }
        }
        "falhar-rede" => {
            imprimirln!("Solicitando falha controlada ao servico de rede...");
            enviar_rede(evento::REDE_FALHAR, 0);
        }
        "ocupar-rede" => {
            imprimirln!("Rede entrara em loop sem ceder; terminal depende de preempcao.");
            enviar_rede(evento::REDE_OCUPAR, 0);
        }
        "violar-memoria" => {
            imprimirln!("Rede tentara ler pagina supervisor; esperado: page fault.");
            enviar_rede(evento::REDE_ACESSAR_NUCLEO, 0);
        }
        cmd => imprimirln!("Comando desconhecido: {}. Digite ajuda.", cmd),
    }
}

fn ler_ip(s: &str) -> Option<[u8; 4]> {
    let mut ip = [0u8; 4];
    let mut partes = s.split('.');
    for octeto in &mut ip {
        *octeto = partes.next()?.parse().ok()?;
    }
    if partes.next().is_some() {
        None
    } else {
        Some(ip)
    }
}

#[panic_handler]
fn panico(info: &core::panic::PanicInfo) -> ! {
    use core::fmt::Write;
    let _ = writeln!(io::Emergencia, "\n[terminal] Panico: {}", info);
    sistema::sair()
}
