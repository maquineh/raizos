//! Driver e1000 e rede mínima em um processo isolado do Raiz OS.
//!
//! Contrato desta versão: QEMU i440fx, Intel 82540EM (8086:100e), um núcleo,
//! preempção pelo microkernel e DMA coerente x86. VA e PA são distintos.
//! Não reutiliza a pilha de rede do firmware depois de ExitBootServices.
//! Não oferece TCP, DHCP, DNS, IPv6, remontagem, opções IPv4 ou sockets gerais.
//! Referências: Intel 8254x Software Developer's Manual; RFCs 768/791/792/826.

use apoio_servicos::{ceder, chamar, receber_com_prazo, tiques};
#[cfg(feature = "teste-rz022-rede")]
use apoio_servicos::{enviar, resolver};
#[cfg(feature = "teste-rz022-rede")]
use contrato_sistema::TERMINAL;
use contrato_sistema::{chamada, erro, prazo, Mensagem, BASE_USUARIO, MMIO_REDE, TAMANHO_USUARIO};
use core::ptr::{addr_of_mut, read_volatile, write_volatile};
use core::sync::atomic::{fence, AtomicBool, Ordering};
use protocolos::{checksum, checksum_udp, escrever_u16, ler_ipv4, ler_u16, ler_udp, montar_ipv4};

const IP: [u8; 4] = [10, 0, 2, 15];
const GATEWAY: [u8; 4] = [10, 0, 2, 2];
const PORTA_ECO: u16 = 7777;
#[cfg(feature = "teste-rx-quota")]
const PORTA_RX_QUOTA: u16 = 7779;
#[cfg(feature = "teste-rx-quota")]
const RX_QUOTA_MAGIC: &[u8; 6] = b"RZ25QT";
#[cfg(feature = "teste-rx-quota")]
const RX_QUOTA_QUANTIDADE: u16 = 18;
#[cfg(feature = "teste-rx-orcamento")]
const PORTA_RX_ORCAMENTO: u16 = 7778;
#[cfg(feature = "teste-rx-orcamento")]
const RX_ORCAMENTO_MAGIC: &[u8; 6] = b"RZ23RX";
#[cfg(feature = "teste-rx-orcamento")]
const RX_ORCAMENTO_QUANTIDADE: u16 = 17;
const QUANTIDADE_RX: usize = 32;
const QUANTIDADE_TX: usize = 8;
const TAMANHO_BUFFER: usize = 2048;
const MTU: usize = 1500;
const IDENTIFICADOR_PING: u16 = 0x524f;
const CAPACIDADE_IPC_INICIAL: usize = 16;

// Preserva mensagens consumidas durante a inicializacao bloqueante do driver.
static mut IPC_INICIAL: [Mensagem; CAPACIDADE_IPC_INICIAL] = [Mensagem {
    origem: 0,
    codigo: 0,
    a: 0,
    b: 0,
    c: 0,
    d: 0,
}; CAPACIDADE_IPC_INICIAL];
static mut IPC_INICIAL_INICIO: usize = 0;
static mut IPC_INICIAL_TAMANHO: usize = 0;
#[cfg(feature = "teste-rz022-rede")]
static mut FIXTURA_RESET_ATIVA: bool = false;
#[cfg(feature = "teste-rz022-rede")]
static mut FIXTURA_STASH_ANUNCIADO: bool = false;

// Os únicos buffers compartilhados com hardware não são expostos como referências
// Rust. O dispositivo recebe PA = base_fisica + (VA - BASE_USUARIO).
#[repr(C, align(128))]
struct AnelRx([DescritorRx; QUANTIDADE_RX]);
#[repr(C, align(128))]
struct AnelTx([DescritorTx; QUANTIDADE_TX]);
#[repr(C, align(16))]
struct BuffersRx([[u8; TAMANHO_BUFFER]; QUANTIDADE_RX]);
#[repr(C, align(16))]
struct BuffersTx([[u8; TAMANHO_BUFFER]; QUANTIDADE_TX]);

#[repr(C)]
#[derive(Clone, Copy)]
struct DescritorRx {
    endereco: u64,
    comprimento: u16,
    checksum: u16,
    estado: u8,
    erros: u8,
    especial: u16,
}

impl DescritorRx {
    const VAZIO: Self = Self {
        endereco: 0,
        comprimento: 0,
        checksum: 0,
        estado: 0,
        erros: 0,
        especial: 0,
    };
}

#[repr(C)]
#[derive(Clone, Copy)]
struct DescritorTx {
    endereco: u64,
    comprimento: u16,
    inicio_checksum: u8,
    comando: u8,
    estado: u8,
    deslocamento_checksum: u8,
    especial: u16,
}

impl DescritorTx {
    const VAZIO: Self = Self {
        endereco: 0,
        comprimento: 0,
        inicio_checksum: 0,
        comando: 0,
        estado: 1,
        deslocamento_checksum: 0,
        especial: 0,
    };
}

static mut ANEL_RX: AnelRx = AnelRx([DescritorRx::VAZIO; QUANTIDADE_RX]);
static mut ANEL_TX: AnelTx = AnelTx([DescritorTx::VAZIO; QUANTIDADE_TX]);
static mut BUFFERS_RX: BuffersRx = BuffersRx([[0; TAMANHO_BUFFER]; QUANTIDADE_RX]);
static mut BUFFERS_TX: BuffersTx = BuffersTx([[0; TAMANHO_BUFFER]; QUANTIDADE_TX]);
static INICIALIZADA: AtomicBool = AtomicBool::new(false);

#[derive(Clone, Copy)]
pub struct Estatisticas {
    pub mac: [u8; 6],
    pub endereco_ipv4: [u8; 4],
    pub recebidos: u64,
    pub transmitidos: u64,
    pub respostas_ping: u64,
    pub ecos_udp: u64,
    #[cfg(feature = "teste-rx-orcamento")]
    pub quadros_rx_orcamento: u64,
    #[cfg(feature = "teste-rx-orcamento")]
    pub mascara_rx_orcamento: u32,
    #[cfg(feature = "teste-rx-quota")]
    pub quadros_rx_quota: u64,
    #[cfg(feature = "teste-rx-quota")]
    pub mascara_rx_quota: u32,
    #[cfg(feature = "teste-rx-quota")]
    pub rx_quota_invalido: bool,
}

#[cfg(feature = "teste-ciclo-espera")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SnapshotCicloEspera {
    pub mmio: u64,
    pub arena_fisica: u64,
    pub indice_rx: u64,
    pub indice_tx: u64,
    pub recebidos: u64,
    pub transmitidos: u64,
}

#[derive(Clone, Copy)]
struct Vizinho {
    ip: [u8; 4],
    mac: [u8; 6],
}

#[derive(Clone, Copy)]
struct PingPendente {
    destino: [u8; 4],
    proximo_salto: [u8; 4],
    sequencia: u16,
}

pub struct Rede {
    mmio: usize,
    base_fisica: u64,
    ultimo_ping_confirmado: Option<PingPendente>,
    erro_ping: bool,
    mac: [u8; 6],
    indice_rx: usize,
    indice_tx: usize,
    identificador_ip: u16,
    sequencia_ping: u16,
    vizinho: Option<Vizinho>,
    ping_pendente: Option<PingPendente>,
    ping_enviado: Option<PingPendente>,
    recebidos: u64,
    transmitidos: u64,
    descartados: u64,
    respostas_ping: u64,
    ecos_udp: u64,
    #[cfg(feature = "teste-rx-orcamento")]
    quadros_rx_orcamento: u64,
    #[cfg(feature = "teste-rx-orcamento")]
    mascara_rx_orcamento: u32,
    #[cfg(feature = "teste-rx-quota")]
    quadros_rx_quota: u64,
    #[cfg(feature = "teste-rx-quota")]
    mascara_rx_quota: u32,
    #[cfg(feature = "teste-rx-quota")]
    proximo_rx_quota: u16,
    #[cfg(feature = "teste-rx-quota")]
    rx_quota_armado: bool,
    #[cfg(feature = "teste-rx-quota")]
    rx_quota_invalido: bool,
    descartando_fragmento_ethernet: bool,
}

fn bdf(barramento: u8, dispositivo: u8, funcao: u8) -> u64 {
    ((barramento as u64) << 16) | ((dispositivo as u64) << 8) | funcao as u64
}

fn pci_ler(barramento: u8, dispositivo: u8, funcao: u8, registro: u8) -> u32 {
    let valor = chamar(
        chamada::PCI_LER,
        bdf(barramento, dispositivo, funcao),
        (registro & 0xfc) as u64,
        0,
        0,
        0,
        0,
    );
    // O microkernel oculta todos os BDFs não atribuídos a este serviço.
    if valor > u32::MAX as u64 {
        u32::MAX
    } else {
        valor as u32
    }
}

fn pci_escrever_comando(
    barramento: u8,
    dispositivo: u8,
    funcao: u8,
    comando: u16,
) -> Result<(), &'static str> {
    let resultado = chamar(
        chamada::PCI_COMANDO,
        bdf(barramento, dispositivo, funcao),
        comando as u64,
        0,
        0,
        0,
        0,
    );
    if resultado == 0 {
        Ok(())
    } else {
        Err("capacidade PCI_COMANDO negada")
    }
}

fn guardar_ipc_inicial(mensagem: Mensagem) -> bool {
    // So a thread de inicializacao acessa o stash; nao ha produtor concorrente.
    unsafe {
        if IPC_INICIAL_TAMANHO == CAPACIDADE_IPC_INICIAL {
            return false;
        }
        let indice = (IPC_INICIAL_INICIO + IPC_INICIAL_TAMANHO) % CAPACIDADE_IPC_INICIAL;
        IPC_INICIAL[indice] = mensagem;
        IPC_INICIAL_TAMANHO += 1;
        true
    }
}

#[cfg(feature = "teste-rz022-rede")]
fn notificar_teste_reset(codigo: u64, a: u64, b: u64, c: u64) -> Result<(), &'static str> {
    let destino = resolver(TERMINAL);
    if destino == 0 || destino == u64::MAX {
        return Err("terminal indisponivel para fixture de reset");
    }
    for _ in 0..64 {
        match enviar(destino, codigo, a, b, c, 0) {
            0 => return Ok(()),
            erro::FILA_CHEIA => ceder(),
            _ => return Err("falha ao notificar fixture de reset"),
        }
    }
    Err("caixa do terminal cheia na fixture de reset")
}

#[cfg(feature = "teste-rz022-rede")]
fn sinalizar_stash_saturado() -> Result<(), &'static str> {
    unsafe {
        if !FIXTURA_RESET_ATIVA
            || FIXTURA_STASH_ANUNCIADO
            || IPC_INICIAL_TAMANHO != CAPACIDADE_IPC_INICIAL
        {
            return Ok(());
        }
        FIXTURA_STASH_ANUNCIADO = true;
    }
    notificar_teste_reset(
        0x7b7,
        CAPACIDADE_IPC_INICIAL as u64,
        resolver(contrato_sistema::REDE),
        tiques(),
    )
}

/// Entrega IPC consumido durante reset antes da caixa do kernel.
pub fn retirar_ipc_inicial() -> Option<Mensagem> {
    unsafe {
        if IPC_INICIAL_TAMANHO == 0 {
            return None;
        }
        let mensagem = IPC_INICIAL[IPC_INICIAL_INICIO];
        IPC_INICIAL_INICIO = (IPC_INICIAL_INICIO + 1) % CAPACIDADE_IPC_INICIAL;
        IPC_INICIAL_TAMANHO -= 1;
        Some(mensagem)
    }
}

/// Espera por prazo guardando IPC consumido. Com o stash cheio, deixa a fila
/// restante no kernel e cede até o prazo; esse é o único fallback com polling.
fn aguardar_evento_ou_prazo(duracao: u64) -> Result<(), &'static str> {
    let inicio = tiques();
    let mut progresso = 0usize;
    let mut prazo_disponivel = true;
    loop {
        let decorrido = tiques().wrapping_sub(inicio);
        if decorrido >= duracao {
            return Ok(());
        }
        let restante = duracao - decorrido;
        let stash_cheio = unsafe { IPC_INICIAL_TAMANHO == CAPACIDADE_IPC_INICIAL };
        if stash_cheio || !prazo_disponivel {
            ceder();
        } else {
            match receber_com_prazo(prazo::FINITO, restante) {
                Ok(mensagem) => {
                    if !guardar_ipc_inicial(mensagem) {
                        return Err("stash IPC inicial saturado durante espera");
                    }
                    #[cfg(feature = "teste-rz022-rede")]
                    sinalizar_stash_saturado()?;
                    progresso = 0;
                    continue;
                }
                Err(erro::TEMPO) => return Ok(()),
                Err(erro::CHAMADA) => {
                    prazo_disponivel = false;
                    ceder();
                }
                Err(_) => return Err("falha ao aguardar prazo do reset e1000"),
            }
        }
        progresso += 1;
        if progresso >= 1_000_000 && tiques().wrapping_sub(inicio) < duracao {
            return Err("timer nao progrediu durante reset e1000");
        }
    }
}

fn aguardar_tiques(quantidade: u64) -> Result<(), &'static str> {
    aguardar_evento_ou_prazo(quantidade)
}

fn aguardar_reset(rede: &Rede, duracao_maxima: u64) -> Result<bool, &'static str> {
    let inicio = tiques();
    let mut progresso = 0usize;
    let mut prazo_disponivel = true;
    loop {
        if rede.ler(0x0000) & (1 << 26) == 0 {
            return Ok(true);
        }
        let decorrido = tiques().wrapping_sub(inicio);
        if decorrido >= duracao_maxima {
            return Ok(false);
        }
        let stash_cheio = unsafe { IPC_INICIAL_TAMANHO == CAPACIDADE_IPC_INICIAL };
        if stash_cheio || !prazo_disponivel {
            ceder();
        } else {
            match receber_com_prazo(prazo::FINITO, 1) {
                Ok(mensagem) => {
                    if !guardar_ipc_inicial(mensagem) {
                        return Err("stash IPC inicial saturado durante reset");
                    }
                    progresso = 0;
                    continue;
                }
                Err(erro::TEMPO) => {}
                Err(erro::CHAMADA) => {
                    prazo_disponivel = false;
                    ceder();
                }
                Err(_) => return Err("falha ao aguardar verificacao do reset e1000"),
            }
        }
        progresso += 1;
        if progresso >= 1_000_000 && tiques().wrapping_sub(inicio) < duracao_maxima {
            return Err("timer nao progrediu durante verificacao do reset e1000");
        }
    }
}

unsafe fn localizar() -> Result<(u8, u8, u8, usize), &'static str> {
    // O perfil QEMU usa barramento PCI 0; os outros são varridos para tolerar bridges.
    for barramento in 0..=255u8 {
        for dispositivo in 0..32u8 {
            for funcao in 0..8u8 {
                let identidade = pci_ler(barramento, dispositivo, funcao, 0);
                if identidade == 0x100e_8086 {
                    let bar = pci_ler(barramento, dispositivo, funcao, 0x10);
                    if bar & 1 != 0 || (bar >> 1) & 3 != 0 {
                        return Err("e1000 requer BAR0 MMIO de 32 bits neste prototipo");
                    }
                    let base = (bar & 0xffff_fff0) as usize;
                    if base == 0 || base > 0xfffe_0000 {
                        return Err("BAR0 e1000 invalido");
                    }
                    return Ok((barramento, dispositivo, funcao, MMIO_REDE as usize));
                }
                if identidade == 0xffff_ffff && funcao == 0 {
                    break;
                }
                if funcao == 0 && (pci_ler(barramento, dispositivo, 0, 0x0c) >> 16) & 0x80 == 0 {
                    break;
                }
            }
        }
    }
    Err("placa e1000 8086:100e nao encontrada")
}

impl Rede {
    /// # Safety
    /// Chamar uma única vez no serviço autorizado. O microkernel deve mapear
    /// a janela de 2 MiB em RAM física contígua e BAR0 em MMIO_REDE. Nenhum
    /// outro serviço pode receber o mesmo dispositivo ou a mesma RAM DMA.
    pub unsafe fn inicializar(base_fisica: u64) -> Result<Self, &'static str> {
        if INICIALIZADA.swap(true, Ordering::AcqRel) {
            return Err("rede ja inicializada");
        }
        if base_fisica == 0
            || base_fisica
                .checked_add(TAMANHO_USUARIO as u64)
                .filter(|&fim| fim <= 0x1_0000_0000)
                .is_none()
        {
            return Err("reserva DMA fisica invalida");
        }
        for (endereco, tamanho) in [
            (addr_of_mut!(ANEL_RX) as u64, core::mem::size_of::<AnelRx>()),
            (addr_of_mut!(ANEL_TX) as u64, core::mem::size_of::<AnelTx>()),
            (
                addr_of_mut!(BUFFERS_RX) as u64,
                core::mem::size_of::<BuffersRx>(),
            ),
            (
                addr_of_mut!(BUFFERS_TX) as u64,
                core::mem::size_of::<BuffersTx>(),
            ),
        ] {
            if endereco < BASE_USUARIO
                || endereco
                    .checked_add(tamanho as u64)
                    .filter(|&fim| fim <= BASE_USUARIO + TAMANHO_USUARIO as u64)
                    .is_none()
            {
                return Err("buffer DMA fora da janela atribuida");
            }
        }
        let (barramento, dispositivo, funcao, mmio) = localizar()?;
        // Manter MMIO disponível e DMA desabilitado durante toda a troca dos anéis.
        // A arena da encarnação anterior permanece em quarentena no microkernel.
        pci_escrever_comando(barramento, dispositivo, funcao, 0x0402)?;
        let mut rede = Self {
            mmio,
            base_fisica,
            ultimo_ping_confirmado: None,
            erro_ping: false,
            mac: [0; 6],
            indice_rx: 0,
            indice_tx: 0,
            identificador_ip: 0,
            sequencia_ping: 0,
            vizinho: None,
            ping_pendente: None,
            ping_enviado: None,
            recebidos: 0,
            transmitidos: 0,
            descartados: 0,
            respostas_ping: 0,
            ecos_udp: 0,
            #[cfg(feature = "teste-rx-orcamento")]
            quadros_rx_orcamento: 0,
            #[cfg(feature = "teste-rx-orcamento")]
            mascara_rx_orcamento: 0,
            #[cfg(feature = "teste-rx-quota")]
            quadros_rx_quota: 0,
            #[cfg(feature = "teste-rx-quota")]
            mascara_rx_quota: 0,
            #[cfg(feature = "teste-rx-quota")]
            proximo_rx_quota: 1,
            #[cfg(feature = "teste-rx-quota")]
            rx_quota_armado: false,
            #[cfg(feature = "teste-rx-quota")]
            rx_quota_invalido: false,
            descartando_fragmento_ethernet: false,
        };
        rede.escrever(0x00d8, u32::MAX); // IMC: nenhuma IRQ neste marco.
        rede.escrever(0x0100, 0); // RCTL
        rede.escrever(0x0400, 0); // TCTL
        let _ = rede.ler(0x0008); // STATUS: flush das escritas MMIO postadas.
        aguardar_tiques(2)?;
        rede.escrever(0x0000, rede.ler(0x0000) | (1 << 26)); // CTRL.RST
        #[cfg(feature = "teste-rz022-rede")]
        if contrato_sistema::geracao_referencia(resolver(contrato_sistema::REDE)) == 1 {
            unsafe {
                FIXTURA_RESET_ATIVA = true;
                FIXTURA_STASH_ANUNCIADO = false;
            }
            notificar_teste_reset(0x7b6, resolver(contrato_sistema::REDE), tiques(), 500)?;
            aguardar_evento_ou_prazo(500)?;
            notificar_teste_reset(0x7b8, 500, resolver(contrato_sistema::REDE), tiques())?;
            unsafe {
                FIXTURA_RESET_ATIVA = false;
            }
        }
        // Intel 8254x §13.4.1: esperar antes de reler o dispositivo após RST.
        aguardar_tiques(2)?;
        let reset_concluido = aguardar_reset(&rede, 50)?;
        if !reset_concluido {
            return Err("e1000 nao concluiu reset");
        }
        rede.escrever(0x00d8, u32::MAX);
        let _ = rede.ler(0x00c0); // limpar ICR
        let baixo = rede.ler(0x5400).to_le_bytes();
        let alto = rede.ler(0x5404);
        rede.mac = [
            baixo[0],
            baixo[1],
            baixo[2],
            baixo[3],
            alto as u8,
            (alto >> 8) as u8,
        ];
        if rede.mac == [0; 6] || rede.mac == [0xff; 6] || rede.mac[0] & 1 != 0 {
            return Err("MAC e1000 invalido");
        }
        rede.escrever(0x5400, u32::from_le_bytes(baixo));
        rede.escrever(0x5404, (alto & 0xffff) | (1 << 31)); // RAH.AV
        for indice in 1..16 {
            rede.escrever(0x5400 + indice * 8, 0);
            rede.escrever(0x5404 + indice * 8, 0);
        }
        for indice in 0..128 {
            rede.escrever(0x5200 + indice * 4, 0); // multicast table
        }
        let ctrl = (rede.ler(0x0000)
            & !((1 << 7)
                | (1 << 3)
                | (1 << 31)
                | (1 << 11)
                | (1 << 12)
                | (1 << 27)
                | (1 << 28)
                | (1 << 30)))
            | (1 << 5)
            | (1 << 6);
        rede.escrever(0x0000, ctrl); // ASDE + SLU; sem forçar velocidade/duplex.
        let rx = addr_of_mut!(ANEL_RX.0).cast::<DescritorRx>();
        let tx = addr_of_mut!(ANEL_TX.0).cast::<DescritorTx>();
        for indice in 0..QUANTIDADE_RX {
            let buffer = addr_of_mut!(BUFFERS_RX.0)
                .cast::<u8>()
                .add(indice * TAMANHO_BUFFER);
            write_volatile(
                rx.add(indice),
                DescritorRx {
                    endereco: rede.endereco_fisico(buffer as usize),
                    ..DescritorRx::VAZIO
                },
            );
        }
        for indice in 0..QUANTIDADE_TX {
            let buffer = addr_of_mut!(BUFFERS_TX.0)
                .cast::<u8>()
                .add(indice * TAMANHO_BUFFER);
            write_volatile(
                tx.add(indice),
                DescritorTx {
                    endereco: rede.endereco_fisico(buffer as usize),
                    ..DescritorTx::VAZIO
                },
            );
        }
        // Publica descritores antes de autorizar o dispositivo a acessá-los.
        fence(Ordering::SeqCst);
        let rx_fisico = rede.endereco_fisico(rx as usize);
        let tx_fisico = rede.endereco_fisico(tx as usize);
        rede.escrever(0x2800, rx_fisico as u32);
        rede.escrever(0x2804, (rx_fisico >> 32) as u32);
        rede.escrever(0x2808, (QUANTIDADE_RX * 16) as u32);
        rede.escrever(0x2810, 0);
        rede.escrever(0x2818, (QUANTIDADE_RX - 1) as u32);
        rede.escrever(0x2820, 0);
        rede.escrever(0x3800, tx_fisico as u32);
        rede.escrever(0x3804, (tx_fisico >> 32) as u32);
        rede.escrever(0x3808, (QUANTIDADE_TX * 16) as u32);
        rede.escrever(0x3810, 0);
        rede.escrever(0x3818, 0);
        rede.escrever(0x3820, 0);
        rede.escrever(0x0410, 10 | (8 << 10) | (6 << 20)); // TIPG
        let _ = rede.ler(0x0008); // Bases/cabeças/caudas novas visíveis antes do DMA.
        pci_escrever_comando(barramento, dispositivo, funcao, 0x0406)?;
        rede.escrever(0x0400, (1 << 1) | (1 << 3) | (15 << 4) | (64 << 12));
        // RX: aceitar unicast próprio/broadcast, buffers 2048, remover CRC Ethernet.
        rede.escrever(0x0100, (1 << 1) | (1 << 15) | (1 << 26));
        Ok(rede)
    }

    fn endereco_fisico(&self, virtual_: usize) -> u64 {
        // Os únicos chamadores usam buffers e anéis estáticos validados no boot.
        self.base_fisica + (virtual_ as u64 - BASE_USUARIO)
    }

    pub fn ultimo_ping(&self) -> Option<([u8; 4], u16)> {
        self.ultimo_ping_confirmado
            .map(|ping| (ping.destino, ping.sequencia))
    }

    pub fn tomar_erro_ping(&mut self) -> bool {
        core::mem::replace(&mut self.erro_ping, false)
    }

    fn ler(&self, registro: usize) -> u32 {
        // SAFETY: BAR validado em inicializar; todos os registros cabem no BAR0.
        unsafe { read_volatile((self.mmio + registro) as *const u32) }
    }

    fn escrever(&self, registro: usize, valor: u32) {
        unsafe { write_volatile((self.mmio + registro) as *mut u32, valor) }
    }

    pub fn estatisticas(&self) -> Estatisticas {
        Estatisticas {
            mac: self.mac,
            endereco_ipv4: IP,
            recebidos: self.recebidos,
            transmitidos: self.transmitidos,
            respostas_ping: self.respostas_ping,
            ecos_udp: self.ecos_udp,
            #[cfg(feature = "teste-rx-orcamento")]
            quadros_rx_orcamento: self.quadros_rx_orcamento,
            #[cfg(feature = "teste-rx-orcamento")]
            mascara_rx_orcamento: self.mascara_rx_orcamento,
            #[cfg(feature = "teste-rx-quota")]
            quadros_rx_quota: self.quadros_rx_quota,
            #[cfg(feature = "teste-rx-quota")]
            mascara_rx_quota: self.mascara_rx_quota,
            #[cfg(feature = "teste-rx-quota")]
            rx_quota_invalido: self.rx_quota_invalido,
        }
    }

    /// Snapshot de campos já mantidos pelo driver; não lê MMIO nem descritores.
    /// A imagem de teste o toma antes/depois de uma espera sem consultar o anel.
    #[cfg(feature = "teste-ciclo-espera")]
    pub fn snapshot_ciclo_espera(&self) -> SnapshotCicloEspera {
        SnapshotCicloEspera {
            mmio: self.mmio as u64,
            arena_fisica: self.base_fisica,
            indice_rx: self.indice_rx as u64,
            indice_tx: self.indice_tx as u64,
            recebidos: self.recebidos,
            transmitidos: self.transmitidos,
        }
    }

    #[cfg(feature = "teste-rx-quota")]
    pub fn armar_rx_quota(&mut self) -> bool {
        if self.rx_quota_armado || self.quadros_rx_quota != 0 || self.mascara_rx_quota != 0 {
            return false;
        }
        self.rx_quota_armado = true;
        self.proximo_rx_quota = 1;
        self.rx_quota_invalido = false;
        true
    }

    /// Conta DD consecutivos sem avançar o anel, apenas na imagem de ensaio.
    /// O serviço não chama `consultar` enquanto espera a mensagem de liberação;
    /// portanto nenhum consumidor de CPU pode limpar estes descritores.
    #[cfg(feature = "teste-rx-orcamento")]
    pub fn rx_orcamento_descritores_prontos(&self) -> (usize, usize) {
        let inicio = self.indice_rx;
        let mut prontos = 0;
        unsafe {
            let anel = addr_of_mut!(ANEL_RX.0).cast::<DescritorRx>();
            for deslocamento in 0..RX_ORCAMENTO_QUANTIDADE as usize {
                let indice = (inicio + deslocamento) % QUANTIDADE_RX;
                let estado = read_volatile(addr_of_mut!((*anel.add(indice)).estado));
                if estado & 1 == 0 {
                    break;
                }
                fence(Ordering::Acquire);
                prontos += 1;
            }
        }
        (prontos, inicio)
    }

    fn transmitir(&mut self, quadro: &[u8]) -> Result<(), &'static str> {
        if quadro.len() < 14 || quadro.len() > 1514 {
            return Err("quadro Ethernet fora do limite");
        }
        unsafe {
            let descritor = addr_of_mut!(ANEL_TX.0)
                .cast::<DescritorTx>()
                .add(self.indice_tx);
            if read_volatile(addr_of_mut!((*descritor).estado)) & 1 == 0 {
                return Err("fila de transmissao ocupada");
            }
            fence(Ordering::Acquire);
            let buffer = addr_of_mut!(BUFFERS_TX.0)
                .cast::<u8>()
                .add(self.indice_tx * TAMANHO_BUFFER);
            for (indice, &byte) in quadro.iter().enumerate() {
                write_volatile(buffer.add(indice), byte);
            }
            write_volatile(
                descritor,
                DescritorTx {
                    endereco: self.endereco_fisico(buffer as usize),
                    comprimento: quadro.len() as u16,
                    inicio_checksum: 0,
                    comando: 0b0000_1011, // EOP | IFCS | RS
                    estado: 0,
                    deslocamento_checksum: 0,
                    especial: 0,
                },
            );
            fence(Ordering::SeqCst);
            self.indice_tx = (self.indice_tx + 1) % QUANTIDADE_TX;
            self.escrever(0x3818, self.indice_tx as u32);
        }
        self.transmitidos += 1;
        Ok(())
    }

    fn receber(&mut self, quadro: &mut [u8; TAMANHO_BUFFER]) -> Option<usize> {
        unsafe {
            let descritor = addr_of_mut!(ANEL_RX.0)
                .cast::<DescritorRx>()
                .add(self.indice_rx);
            let estado = read_volatile(addr_of_mut!((*descritor).estado));
            if estado & 1 == 0 {
                return None;
            }
            fence(Ordering::Acquire);
            let comprimento = read_volatile(addr_of_mut!((*descritor).comprimento)) as usize;
            let erros = read_volatile(addr_of_mut!((*descritor).erros));
            let fim_pacote = estado & 2 != 0;
            let valido = fim_pacote
                && !self.descartando_fragmento_ethernet
                && erros == 0
                && (14..=1514).contains(&comprimento);
            self.descartando_fragmento_ethernet = !fim_pacote;
            if valido {
                let buffer = addr_of_mut!(BUFFERS_RX.0)
                    .cast::<u8>()
                    .add(self.indice_rx * TAMANHO_BUFFER);
                for (indice, byte) in quadro[..comprimento].iter_mut().enumerate() {
                    *byte = read_volatile(buffer.add(indice));
                }
                self.recebidos += 1;
            } else {
                self.descartados += 1;
            }
            write_volatile(addr_of_mut!((*descritor).estado), 0);
            fence(Ordering::SeqCst);
            self.escrever(0x2818, self.indice_rx as u32);
            self.indice_rx = (self.indice_rx + 1) % QUANTIDADE_RX;
            // Zero indica descritor consumido mas quadro rejeitado.
            Some(if valido { comprimento } else { 0 })
        }
    }

    /// Processa até 16 quadros; `true` pede cessao e nova rodada sem dormir.
    pub fn consultar(&mut self) -> bool {
        let mut quadro = [0u8; TAMANHO_BUFFER];
        for _ in 0..16 {
            match self.receber(&mut quadro) {
                Some(0) => continue,
                Some(comprimento) => self.processar(&quadro[..comprimento]),
                None => return false,
            }
        }
        true
    }

    fn processar(&mut self, quadro: &[u8]) {
        if quadro[..6] != self.mac && quadro[..6] != [0xff; 6] {
            self.descartados += 1;
            return;
        }
        let mac_origem = [
            quadro[6], quadro[7], quadro[8], quadro[9], quadro[10], quadro[11],
        ];
        if mac_origem[0] & 1 != 0 || mac_origem == [0; 6] {
            self.descartados += 1;
            return;
        }
        match ler_u16(quadro, 12) {
            0x0806 => self.processar_arp(&quadro[14..], mac_origem),
            0x0800 => self.processar_ipv4(&quadro[14..], mac_origem),
            _ => self.descartados += 1,
        }
    }

    fn enviar_arp(
        &mut self,
        operacao: u16,
        destino_ip: [u8; 4],
        destino_mac: [u8; 6],
    ) -> Result<(), &'static str> {
        let mut quadro = [0u8; 42];
        quadro[..6].copy_from_slice(&destino_mac);
        quadro[6..12].copy_from_slice(&self.mac);
        escrever_u16(&mut quadro, 12, 0x0806);
        escrever_u16(&mut quadro, 14, 1); // Ethernet
        escrever_u16(&mut quadro, 16, 0x0800); // IPv4
        quadro[18] = 6;
        quadro[19] = 4;
        escrever_u16(&mut quadro, 20, operacao);
        quadro[22..28].copy_from_slice(&self.mac);
        quadro[28..32].copy_from_slice(&IP);
        if operacao == 2 {
            quadro[32..38].copy_from_slice(&destino_mac);
        }
        quadro[38..42].copy_from_slice(&destino_ip);
        self.transmitir(&quadro)
    }

    fn processar_arp(&mut self, arp: &[u8], mac_ethernet: [u8; 6]) {
        if arp.len() < 28
            || ler_u16(arp, 0) != 1
            || ler_u16(arp, 2) != 0x0800
            || arp[4] != 6
            || arp[5] != 4
            || arp[8..14] != mac_ethernet
        {
            self.descartados += 1;
            return;
        }
        let operacao = ler_u16(arp, 6);
        let origem = [arp[14], arp[15], arp[16], arp[17]];
        if arp[24..28] != IP || (operacao != 1 && operacao != 2) {
            self.descartados += 1;
            return;
        }
        if operacao == 1 {
            if self.enviar_arp(2, origem, mac_ethernet).is_err() {
                self.descartados += 1;
            }
        }
        // Cache de uma entrada; somente aceita resposta ao próximo salto aguardado.
        if operacao == 2 && arp[18..24] == self.mac {
            if let Some(pendente) = self.ping_pendente {
                if pendente.proximo_salto == origem {
                    self.vizinho = Some(Vizinho {
                        ip: origem,
                        mac: mac_ethernet,
                    });
                    self.ping_pendente = None;
                    if self.enviar_ping(pendente, mac_ethernet).is_err() {
                        self.erro_ping = true;
                    }
                }
            }
        }
    }

    fn enviar_ip(
        &mut self,
        destino_mac: [u8; 6],
        destino_ip: [u8; 4],
        protocolo: u8,
        carga: &[u8],
    ) -> Result<(), &'static str> {
        if carga.len() > MTU - 20 {
            return Err("datagrama excede MTU 1500");
        }
        let mut quadro = [0u8; 1514];
        quadro[..6].copy_from_slice(&destino_mac);
        quadro[6..12].copy_from_slice(&self.mac);
        escrever_u16(&mut quadro, 12, 0x0800);
        self.identificador_ip = self.identificador_ip.wrapping_add(1);
        let tamanho = montar_ipv4(
            &mut quadro[14..],
            IP,
            destino_ip,
            protocolo,
            self.identificador_ip,
            carga,
        )
        .map_err(|_| "datagrama IPv4 invalido")?;
        self.transmitir(&quadro[..14 + tamanho])
    }

    fn processar_ipv4(&mut self, bytes: &[u8], mac_origem: [u8; 6]) {
        let ip = match ler_ipv4(bytes) {
            Ok(ip) => ip,
            Err(_) => {
                self.descartados += 1;
                return;
            }
        };
        // O serviço de laboratório responde somente ao seu IP unicast.
        if ip.destino != IP || ip.origem == [0; 4] || ip.origem[0] >= 224 {
            self.descartados += 1;
            return;
        }
        match ip.protocolo {
            1 => {
                if ip.carga.len() < 8
                    || ip.carga.len() > MTU - 20
                    || checksum(ip.carga) != 0
                    || ip.carga[1] != 0
                {
                    self.descartados += 1;
                    return;
                }
                if ip.carga[0] == 8 {
                    let mut resposta = [0u8; MTU - 20];
                    let tamanho = ip.carga.len();
                    resposta[..tamanho].copy_from_slice(ip.carga);
                    resposta[0] = 0;
                    escrever_u16(&mut resposta, 2, 0);
                    let soma = checksum(&resposta[..tamanho]);
                    escrever_u16(&mut resposta, 2, soma);
                    if self
                        .enviar_ip(mac_origem, ip.origem, 1, &resposta[..tamanho])
                        .is_err()
                    {
                        self.descartados += 1;
                    }
                } else if ip.carga[0] == 0 {
                    if let Some(pendente) = self.ping_enviado {
                        if ip.origem == pendente.destino
                            && ler_u16(ip.carga, 4) == IDENTIFICADOR_PING
                            && ler_u16(ip.carga, 6) == pendente.sequencia
                        {
                            self.ping_enviado = None;
                            self.respostas_ping += 1;
                            self.ultimo_ping_confirmado = Some(pendente);
                        } else {
                            self.descartados += 1;
                        }
                    } else {
                        self.descartados += 1;
                    }
                } else {
                    self.descartados += 1;
                }
            }
            17 => {
                let udp = match ler_udp(&ip) {
                    Ok(udp) => udp,
                    Err(_) => {
                        self.descartados += 1;
                        return;
                    }
                };
                if udp.porta_destino != PORTA_ECO
                    && !cfg!(feature = "teste-rx-orcamento")
                    && !cfg!(feature = "teste-rx-quota")
                {
                    self.descartados += 1;
                    return;
                }
                #[cfg(feature = "teste-rx-quota")]
                if udp.porta_destino == PORTA_RX_QUOTA {
                    if udp.porta_origem == 0
                        || udp.carga.len() != 8
                        || &udp.carga[..6] != RX_QUOTA_MAGIC
                    {
                        self.rx_quota_invalido = true;
                        self.descartados += 1;
                        return;
                    }
                    let sequencia = u16::from_be_bytes([udp.carga[6], udp.carga[7]]);
                    if sequencia == 0 {
                        self.responder_udp(
                            mac_origem,
                            ip.origem,
                            PORTA_RX_QUOTA,
                            udp.porta_origem,
                            udp.carga,
                        );
                        return;
                    }
                    if !self.rx_quota_armado
                        || sequencia != self.proximo_rx_quota
                        || sequencia > RX_QUOTA_QUANTIDADE
                    {
                        self.rx_quota_invalido = true;
                        self.descartados += 1;
                        return;
                    }
                    self.quadros_rx_quota = self.quadros_rx_quota.saturating_add(1);
                    self.mascara_rx_quota |= 1u32 << (sequencia - 1);
                    self.proximo_rx_quota += 1;
                    return;
                }
                #[cfg(feature = "teste-rx-quota")]
                if udp.porta_destino != PORTA_ECO && udp.porta_destino != PORTA_RX_QUOTA {
                    self.descartados += 1;
                    return;
                }
                #[cfg(feature = "teste-rx-orcamento")]
                if udp.porta_destino == PORTA_RX_ORCAMENTO {
                    if udp.porta_origem == 0
                        || udp.carga.len() != 8
                        || &udp.carga[..6] != RX_ORCAMENTO_MAGIC
                    {
                        self.descartados += 1;
                        return;
                    }
                    let sequencia = u16::from_be_bytes([udp.carga[6], udp.carga[7]]);
                    if sequencia >= RX_ORCAMENTO_QUANTIDADE {
                        self.descartados += 1;
                        return;
                    }
                    self.quadros_rx_orcamento += 1;
                    self.mascara_rx_orcamento |= 1u32 << sequencia;
                    return;
                }
                if udp.porta_destino != PORTA_ECO
                    || udp.porta_origem == 0
                    || udp.carga.len() > MTU - 28
                {
                    self.descartados += 1;
                    return;
                }
                self.responder_udp(
                    mac_origem,
                    ip.origem,
                    PORTA_ECO,
                    udp.porta_origem,
                    udp.carga,
                );
            }
            _ => self.descartados += 1,
        }
    }

    fn responder_udp(
        &mut self,
        mac: [u8; 6],
        destino: [u8; 4],
        porta_origem: u16,
        porta_destino: u16,
        carga: &[u8],
    ) {
        let mut resposta = [0u8; MTU - 20];
        let tamanho = carga.len() + 8;
        escrever_u16(&mut resposta, 0, porta_origem);
        escrever_u16(&mut resposta, 2, porta_destino);
        escrever_u16(&mut resposta, 4, tamanho as u16);
        resposta[8..tamanho].copy_from_slice(carga);
        let soma = checksum_udp(IP, destino, &resposta[..tamanho]);
        escrever_u16(&mut resposta, 6, if soma == 0 { 0xffff } else { soma });
        match self.enviar_ip(mac, destino, 17, &resposta[..tamanho]) {
            Ok(()) => self.ecos_udp += 1,
            Err(_) => self.descartados += 1,
        }
    }

    /// Envia um eco assíncrono. A resposta é processada por `consultar`.
    /// Uma nova chamada substitui o eco anterior; ainda não há timers/retransmissão.
    pub fn ping(&mut self, destino: [u8; 4]) -> Result<(), &'static str> {
        if destino == IP
            || destino[0] == 0
            || destino[0] >= 224
            || destino[0] == 127
            || (destino[..3] == IP[..3] && (destino[3] == 0 || destino[3] == 255))
        {
            return Err("destino ping precisa ser IPv4 unicast remoto");
        }
        let proximo_salto = if destino[..3] == IP[..3] {
            destino
        } else {
            GATEWAY
        };
        self.sequencia_ping = self.sequencia_ping.wrapping_add(1);
        let pendente = PingPendente {
            destino,
            proximo_salto,
            sequencia: self.sequencia_ping,
        };
        self.ping_pendente = None;
        self.ping_enviado = None;
        if let Some(vizinho) = self.vizinho {
            if vizinho.ip == proximo_salto {
                return self.enviar_ping(pendente, vizinho.mac);
            }
        }
        self.enviar_arp(1, proximo_salto, [0xff; 6])?;
        self.ping_pendente = Some(pendente);
        Ok(())
    }

    fn enviar_ping(&mut self, pendente: PingPendente, mac: [u8; 6]) -> Result<(), &'static str> {
        let mut eco = [0u8; 24];
        eco[0] = 8;
        escrever_u16(&mut eco, 4, IDENTIFICADOR_PING);
        escrever_u16(&mut eco, 6, pendente.sequencia);
        eco[8..].copy_from_slice(b"RAIZ-OS-RUST-001");
        let soma = checksum(&eco);
        escrever_u16(&mut eco, 2, soma);
        self.enviar_ip(mac, pendente.destino, 1, &eco)?;
        self.ping_enviado = Some(pendente);
        Ok(())
    }
}
