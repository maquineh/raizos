//! Filas limitadas, referencias de instancia e autorizacao estatica.
//! Nenhuma funcao deste modulo acessa hardware ou memoria global mutavel.
use crate::agendamento::{EsperaIpc, Estado};
use contrato_sistema::{
    criar_referencia, erro, geracao_referencia, id_referencia, referencia_valida, Mensagem,
    CAPACIDADE_CAIXA_IPC, LIMITE_ORIGEM_CAIXA_TERMINAL, MAX_INSTANCIAS_REDE, QUANTIDADE_SERVICOS,
    REDE, SUPERVISOR, TERMINAL,
};

/// Chamador valida identidade, autoridade e vida antes desta operacao atomica.
/// A espera esta associada ao proprio Processo, substituido junto a referencia.
/// Prazos sao varridos no slot publicado; callbacks ou filas assincronas futuras
/// precisam identificar instancia e operacao, sem confiar apenas no indice.
pub fn inserir_e_despertar(
    fila: &mut CaixaMensagens,
    espera: &mut EsperaIpc,
    estado: &mut Estado,
    mensagem: Mensagem,
) -> Result<bool, u64> {
    fila.inserir(mensagem)?;
    Ok(espera.mensagem_admitida(estado))
}

/// Admissao e despertar compartilham a mesma exclusao do chamador.
pub fn inserir_terminal_e_despertar(
    fila: &mut CaixaMensagens,
    espera: &mut EsperaIpc,
    estado: &mut Estado,
    mensagem: Mensagem,
) -> Result<bool, u64> {
    fila.admitir_origem_terminal(mensagem.origem)?;
    inserir_e_despertar(fila, espera, estado, mensagem)
}

/// Remove origens revogadas antes de retornar a primeira mensagem elegivel.
pub fn retirar_atual(
    fila: &mut CaixaMensagens,
    atuais: &[u64; QUANTIDADE_SERVICOS],
) -> Option<Mensagem> {
    while let Some(mensagem) = fila.retirar() {
        if localizar_instancia(mensagem.origem, atuais).is_ok() {
            return Some(mensagem);
        }
    }
    None
}

/// Na selecao de uma recepcao pendente, consome ou volta a bloquear se a
/// revogacao removeu tudo. O chamador deve serializar com insercao e purga.
pub fn preparar_recepcao(
    fila: &mut CaixaMensagens,
    espera: &mut EsperaIpc,
    estado: &mut Estado,
    atuais: &[u64; QUANTIDADE_SERVICOS],
) -> Result<Option<Mensagem>, u64> {
    if let Some(m) = retirar_atual(fila, atuais) {
        return Ok(Some(m));
    }
    match espera.registrar(estado, 0)? {
        true => Ok(None),
        false => Err(erro::ESTADO),
    }
}
pub const CAPACIDADE: usize = CAPACIDADE_CAIXA_IPC;
const VAZIA: Mensagem = Mensagem {
    origem: 0,
    codigo: 0,
    a: 0,
    b: 0,
    c: 0,
    d: 0,
};

#[derive(Clone, Copy)]
pub struct CaixaMensagens {
    mensagens: [Mensagem; CAPACIDADE],
    inicio: usize,
    tamanho: usize,
}
impl CaixaMensagens {
    pub const NOVA: Self = Self {
        mensagens: [VAZIA; CAPACIDADE],
        inicio: 0,
        tamanho: 0,
    };
    pub fn inserir(&mut self, mensagem: Mensagem) -> Result<(), u64> {
        if self.tamanho == CAPACIDADE {
            return Err(erro::FILA_CHEIA);
        }
        self.mensagens[(self.inicio + self.tamanho) % CAPACIDADE] = mensagem;
        self.tamanho += 1;
        Ok(())
    }
    /// Conta pelo ID logico: geracoes distintas nao multiplicam a quota.
    /// A purga continua identificando a instancia exata.
    pub fn admitir_origem_terminal(&self, origem: u64) -> Result<(), u64> {
        let id = id_referencia(origem);
        let quantidade = (0..self.tamanho)
            .filter(|&i| id_referencia(self.mensagens[(self.inicio + i) % CAPACIDADE].origem) == id)
            .count();
        if quantidade >= LIMITE_ORIGEM_CAIXA_TERMINAL {
            Err(erro::FILA_CHEIA)
        } else {
            Ok(())
        }
    }
    pub fn retirar(&mut self) -> Option<Mensagem> {
        if self.tamanho == 0 {
            return None;
        }
        let mensagem = self.mensagens[self.inicio];
        self.mensagens[self.inicio] = VAZIA;
        self.inicio = (self.inicio + 1) % CAPACIDADE;
        self.tamanho -= 1;
        Some(mensagem)
    }
    pub fn limpar(&mut self) -> usize {
        let removidas = self.tamanho;
        *self = Self::NOVA;
        removidas
    }
    /// Mantem ordem e mensagens de outras geracoes do mesmo servico.
    pub fn descartar_origem(&mut self, referencia: u64) -> usize {
        let original = self.tamanho;
        let mut nova = Self::NOVA;
        while let Some(mensagem) = self.retirar() {
            if mensagem.origem != referencia {
                // O numero preservado nunca supera o tamanho original da fila.
                let _ = nova.inserir(mensagem);
            }
        }
        let removidas = original - nova.tamanho;
        *self = nova;
        removidas
    }
    pub fn tamanho(&self) -> usize {
        self.tamanho
    }
    /// Oraculo de composicao T03-12; somente a imagem opt-in o consulta.
    #[cfg(feature = "teste-rx-quota")]
    pub fn rx_quota_confere(&self, rede: u64, supervisor: u64, controle_presente: bool) -> bool {
        let esperado = if controle_presente { 16 } else { 15 };
        if self.tamanho != esperado {
            return false;
        }
        for sequencia in 1..=15u64 {
            let mensagem = self.mensagens[(self.inicio + sequencia as usize - 1) % CAPACIDADE];
            if mensagem.origem != rede
                || mensagem.codigo != 0x7ba
                || mensagem.a != sequencia
                || mensagem.b != 0x525a32355154
                || mensagem.c != 0
                || mensagem.d != 0
            {
                return false;
            }
        }
        if controle_presente {
            let mensagem = self.mensagens[(self.inicio + 15) % CAPACIDADE];
            return mensagem.origem == supervisor
                && mensagem.codigo == 0x7be
                && mensagem.a == 1
                && mensagem.b == 0x44
                && mensagem.c == 0x55
                && mensagem.d == 0x66;
        }
        true
    }
    /// Conta mensagens que `retirar_atual` poderia devolver, sem alterar a FIFO.
    #[cfg(any(feature = "teste-seletor-irq-ipc", feature = "teste-irq-fifo-cheia"))]
    pub fn contar_atual(&self, atuais: &[u64; QUANTIDADE_SERVICOS]) -> usize {
        (0..self.tamanho)
            .filter(|&i| {
                localizar_instancia(
                    self.mensagens[(self.inicio + i) % CAPACIDADE].origem,
                    atuais,
                )
                .is_ok()
            })
            .count()
    }
    /// Confere os dois primeiros eventos elegíveis, preservando a FIFO.
    #[cfg(feature = "teste-seletor-irq-ipc")]
    pub fn prefixo_atual_confere(
        &self,
        atuais: &[u64; QUANTIDADE_SERVICOS],
        origem: u64,
        codigo: u64,
        primeiro: u64,
        segundo: u64,
    ) -> bool {
        let esperados = [primeiro, segundo];
        let mut encontrados = 0;
        for i in 0..self.tamanho {
            let mensagem = self.mensagens[(self.inicio + i) % CAPACIDADE];
            if localizar_instancia(mensagem.origem, atuais).is_err() {
                continue;
            }
            if encontrados == esperados.len()
                || mensagem.origem != origem
                || mensagem.codigo != codigo
                || mensagem.a != esperados[encontrados]
                || mensagem.b != 0
                || mensagem.c != 0
                || mensagem.d != 0
            {
                return false;
            }
            encontrados += 1;
        }
        encontrados == esperados.len()
    }
    /// Sonda opt-in: valida a caixa completa em ordem de anel sem consumi-la.
    #[cfg(feature = "teste-irq-fifo-cheia")]
    pub fn fila_irq_teste_confere(
        &self,
        rede: u64,
        supervisor: u64,
        quantidade_rede: usize,
        supervisor_presente: bool,
    ) -> bool {
        if self.tamanho != quantidade_rede + usize::from(supervisor_presente)
            || self.tamanho > CAPACIDADE
        {
            return false;
        }
        for n in 0..quantidade_rede {
            let m = self.mensagens[(self.inicio + n) % CAPACIDADE];
            if m.origem != rede
                || m.codigo != 0x7da
                || m.a != n as u64 + 1
                || m.b != 0x11
                || m.c != 0x22
                || m.d != 0x33
            {
                return false;
            }
        }
        if !supervisor_presente {
            return true;
        }
        let m = self.mensagens[(self.inicio + quantidade_rede) % CAPACIDADE];
        m.origem == supervisor
            && m.codigo == 0x7db
            && m.a == 1
            && m.b == 0x44
            && m.c == 0x55
            && m.d == 0x66
    }
}

pub fn localizar_instancia(
    referencia: u64,
    atuais: &[u64; QUANTIDADE_SERVICOS],
) -> Result<usize, u64> {
    if !referencia_valida(referencia) {
        return Err(erro::SERVICO);
    }
    let indice = id_referencia(referencia) as usize - 1;
    if atuais[indice] == 0 {
        return Err(erro::SERVICO);
    }
    if atuais[indice] != referencia {
        return Err(erro::REFERENCIA_ANTIGA);
    }
    Ok(indice)
}

pub fn pode_enviar(origem: u64, destino: u64) -> bool {
    (origem == TERMINAL && (destino == REDE || destino == SUPERVISOR))
        || (destino == TERMINAL && (origem == REDE || origem == SUPERVISOR))
}

pub fn autorizar_gerenciamento(
    chamador: u64,
    referencia: u64,
    atuais: &[u64; QUANTIDADE_SERVICOS],
) -> Result<usize, u64> {
    // A autoridade e verificada antes de examinar o alvo solicitado.
    if chamador != SUPERVISOR {
        return Err(erro::PERMISSAO);
    }
    let indice = localizar_instancia(referencia, atuais)?;
    if id_referencia(referencia) != REDE {
        return Err(erro::PERMISSAO);
    }
    Ok(indice)
}

pub fn proxima_instancia_rede(atual: u64) -> Result<u64, u64> {
    if !referencia_valida(atual) {
        return Err(erro::SERVICO);
    }
    if id_referencia(atual) != REDE {
        return Err(erro::PERMISSAO);
    }
    let geracao = geracao_referencia(atual);
    if geracao >= MAX_INSTANCIAS_REDE {
        return Err(erro::RECURSOS);
    }
    criar_referencia(REDE, geracao + 1).ok_or(erro::RECURSOS)
}

/// Quota e preparacao antecedem qualquer revogacao do processo publicado.
pub fn preparar_reinicio<T>(
    atual: u64,
    preparar: impl FnOnce(u64) -> Result<T, u64>,
) -> Result<(u64, T), u64> {
    let nova = proxima_instancia_rede(atual)?;
    let imagem = preparar(nova)?;
    Ok((nova, imagem))
}

/// Duas arenas estaticas e uma arena diferente para cada encarnacao da rede.
/// Uma arena que ja hospedou a rede nunca recebe outra instancia durante este boot.
pub fn arena_da_instancia(referencia: u64) -> Result<usize, u64> {
    if !referencia_valida(referencia) {
        return Err(erro::SERVICO);
    }
    let geracao = geracao_referencia(referencia);
    match id_referencia(referencia) {
        TERMINAL if geracao == 1 => Ok(0),
        SUPERVISOR if geracao == 1 => Ok(1),
        REDE if geracao <= MAX_INSTANCIAS_REDE => Ok(2 + geracao as usize - 1),
        REDE => Err(erro::RECURSOS),
        _ => Err(erro::REFERENCIA_ANTIGA),
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    fn referencia(id: u64, geracao: u64) -> u64 {
        criar_referencia(id, geracao).unwrap()
    }
    fn mensagem(origem: u64, sequencia: u64) -> Mensagem {
        Mensagem {
            origem,
            codigo: 1,
            a: sequencia,
            b: 0,
            c: 0,
            d: 0,
        }
    }
    #[cfg(feature = "teste-rx-quota")]
    #[test]
    fn sonda_rx_quota_confere_fifo_real_e_origem_exata_do_controle() {
        let rede = referencia(REDE, 1);
        let supervisor = referencia(SUPERVISOR, 1);
        let mut fila = CaixaMensagens::NOVA;
        for sequencia in 1..=15u64 {
            fila.inserir(Mensagem {
                origem: rede,
                codigo: 0x7ba,
                a: sequencia,
                b: 0x525a32355154,
                c: 0,
                d: 0,
            })
            .unwrap();
        }
        assert!(fila.rx_quota_confere(rede, supervisor, false));
        assert!(!fila.rx_quota_confere(rede, supervisor, true));
        fila.inserir(Mensagem {
            origem: supervisor,
            codigo: 0x7be,
            a: 1,
            b: 0x44,
            c: 0x55,
            d: 0x66,
        })
        .unwrap();
        assert!(fila.rx_quota_confere(rede, supervisor, true));
        assert!(!fila.rx_quota_confere(rede, referencia(SUPERVISOR, 2), true));

        let mut ordem_errada = fila;
        ordem_errada.mensagens[(ordem_errada.inicio + 7) % CAPACIDADE].a = 9;
        assert!(!ordem_errada.rx_quota_confere(rede, supervisor, true));
        let mut payload_errado = fila;
        payload_errado.mensagens[(payload_errado.inicio + 15) % CAPACIDADE].c = 0x56;
        assert!(!payload_errado.rx_quota_confere(rede, supervisor, true));
    }

    #[cfg(feature = "teste-seletor-irq-ipc")]
    #[test]
    fn sondagem_fifo_nao_consumitiva_filtra_geracao_antiga_e_respeita_ordem_circular() {
        let terminal = referencia(TERMINAL, 1);
        let rede_antiga = referencia(REDE, 1);
        let rede_atual = referencia(REDE, 2);
        let atuais = [terminal, rede_atual, referencia(SUPERVISOR, 1)];
        let mut fila = CaixaMensagens::NOVA;
        for sequencia in 0..CAPACIDADE - 2 {
            fila.inserir(mensagem(terminal, sequencia as u64)).unwrap();
        }
        for _ in 0..CAPACIDADE - 2 {
            fila.retirar().unwrap();
        }
        fila.inserir(mensagem(rede_antiga, 99)).unwrap();
        fila.inserir(mensagem(rede_atual, 1)).unwrap();
        fila.inserir(mensagem(rede_atual, 2)).unwrap();
        let tamanho = fila.tamanho();

        assert_eq!(fila.contar_atual(&atuais), 2);
        assert!(fila.prefixo_atual_confere(&atuais, rede_atual, 1, 1, 2));
        assert!(!fila.prefixo_atual_confere(&atuais, rede_atual, 1, 2, 1));
        assert_eq!(fila.tamanho(), tamanho);
    }
    #[test]
    fn falhas_de_preparacao_ou_quota_preservam_espera_publicada() {
        use crate::agendamento::{cancelar_espera, expirar_recepcao};
        use core::cell::Cell;
        let mut estado = Estado::ATIVO;
        let mut espera = EsperaIpc::NOVA;
        let mut pendente = true;
        let mut prazo = Some(10);
        assert_eq!(espera.registrar(&mut estado, 0), Ok(true));
        let executou = Cell::new(false);
        assert_eq!(
            preparar_reinicio(referencia(REDE, 4), |_| {
                executou.set(true);
                Ok(())
            }),
            Err(erro::RECURSOS)
        );
        assert!(!executou.get());
        assert_eq!(
            preparar_reinicio(referencia(REDE, 1), |_| Err::<(), _>(erro::IMAGEM)),
            Err(erro::IMAGEM)
        );
        assert!(!estado.executavel());
        assert!(pendente);
        assert_eq!(prazo, Some(10));
        let (nova, ()) = preparar_reinicio(referencia(REDE, 1), |_| Ok(())).unwrap();
        cancelar_espera(&mut espera, &mut pendente, &mut prazo);
        estado.terminar(crate::agendamento::Vida::Finalizada);
        assert_eq!(nova, referencia(REDE, 2));
        assert!(!expirar_recepcao(
            10,
            &mut prazo,
            &mut pendente,
            &mut espera,
            &mut estado
        ));
        let mut nova_espera = EsperaIpc::NOVA;
        let mut novo_estado = Estado::ATIVO;
        let mut novo_pendente = true;
        let mut novo_prazo = Some(20);
        assert_eq!(nova_espera.registrar(&mut novo_estado, 0), Ok(true));
        assert!(!expirar_recepcao(
            10,
            &mut novo_prazo,
            &mut novo_pendente,
            &mut nova_espera,
            &mut novo_estado
        ));
        assert!(!novo_estado.executavel());
    }
    #[test]
    fn observacao_one_shot_respeita_geracao_prazo_e_fifo() {
        use crate::agendamento::{expirar_recepcao, invalidar_espera_por_alvo};
        let antiga = referencia(REDE, 1);
        let nova = referencia(REDE, 2);
        let supervisor = referencia(SUPERVISOR, 1);
        let mut fila = CaixaMensagens::NOVA;
        let mut estado = Estado::ATIVO;
        let mut espera = EsperaIpc::NOVA;
        let mut pendente = true;
        let mut prazo = Some(10);
        let mut observado = antiga;
        assert_eq!(espera.registrar(&mut estado, 0), Ok(true));
        assert!(!invalidar_espera_por_alvo(
            nova,
            &mut observado,
            &mut espera,
            &mut pendente,
            &mut prazo,
            &mut estado
        ));
        assert_eq!(
            inserir_e_despertar(&mut fila, &mut espera, &mut estado, mensagem(supervisor, 1)),
            Ok(true)
        );
        assert!(!expirar_recepcao(
            9,
            &mut prazo,
            &mut pendente,
            &mut espera,
            &mut estado
        ));
        assert!(invalidar_espera_por_alvo(
            antiga,
            &mut observado,
            &mut espera,
            &mut pendente,
            &mut prazo,
            &mut estado
        ));
        assert!(!invalidar_espera_por_alvo(
            antiga,
            &mut observado,
            &mut espera,
            &mut pendente,
            &mut prazo,
            &mut estado
        ));
        assert_eq!(fila.retirar().unwrap().a, 1);
        assert!(estado.executavel());
        assert_eq!(espera.registrar(&mut estado, 0), Ok(true));
        pendente = true;
        prazo = Some(20);
        observado = nova;
        assert!(expirar_recepcao(
            20,
            &mut prazo,
            &mut pendente,
            &mut espera,
            &mut estado
        ));
        assert!(!invalidar_espera_por_alvo(
            nova,
            &mut observado,
            &mut espera,
            &mut pendente,
            &mut prazo,
            &mut estado
        ));
    }
    #[test]
    fn empate_vence_prazo_sem_consumir_fifo_e_conclusao_anterior_persiste() {
        use crate::agendamento::expirar_recepcao;
        let terminal = referencia(TERMINAL, 1);
        let atuais = [terminal, referencia(REDE, 1), referencia(SUPERVISOR, 1)];
        let mut fila = CaixaMensagens::NOVA;
        let mut estado = Estado::ATIVO;
        let mut espera = EsperaIpc::NOVA;
        let mut pendente = true;
        let mut prazo = Some(10);
        assert_eq!(espera.registrar(&mut estado, 0), Ok(true));
        assert_eq!(
            inserir_e_despertar(&mut fila, &mut espera, &mut estado, mensagem(terminal, 7)),
            Ok(true)
        );
        assert!(expirar_recepcao(
            10,
            &mut prazo,
            &mut pendente,
            &mut espera,
            &mut estado
        ));
        assert_eq!(retirar_atual(&mut fila, &atuais).unwrap().a, 7);
        assert_eq!(espera.registrar(&mut estado, 0), Ok(true));
        pendente = true;
        prazo = Some(20);
        assert_eq!(
            inserir_e_despertar(&mut fila, &mut espera, &mut estado, mensagem(terminal, 8)),
            Ok(true)
        );
        assert!(!expirar_recepcao(
            19,
            &mut prazo,
            &mut pendente,
            &mut espera,
            &mut estado
        ));
        let recebida = preparar_recepcao(&mut fila, &mut espera, &mut estado, &atuais)
            .unwrap()
            .unwrap();
        pendente = false;
        prazo = None;
        assert!(!expirar_recepcao(
            20,
            &mut prazo,
            &mut pendente,
            &mut espera,
            &mut estado
        ));
        assert_eq!(recebida.a, 8);
    }
    #[test]
    fn recepcao_pendente_rearma_apos_revogacao_e_mantem_fifo_valida() {
        let velha = referencia(REDE, 1);
        let nova = referencia(REDE, 2);
        let terminal = referencia(TERMINAL, 1);
        let supervisor = referencia(SUPERVISOR, 1);
        let atuais = [terminal, nova, supervisor];
        let mut fila = CaixaMensagens::NOVA;
        let mut estado = Estado::ATIVO;
        let mut espera = EsperaIpc::NOVA;
        assert_eq!(espera.registrar(&mut estado, 0), Ok(true));
        assert_eq!(
            inserir_e_despertar(&mut fila, &mut espera, &mut estado, mensagem(velha, 1)),
            Ok(true)
        );
        assert_eq!(fila.descartar_origem(velha), 1);
        assert!(
            preparar_recepcao(&mut fila, &mut espera, &mut estado, &atuais)
                .unwrap()
                .is_none()
        );
        assert!(!estado.executavel());
        let mut especial = mensagem(supervisor, 2);
        especial.codigo = erro::CHAMADA;
        assert_eq!(
            inserir_e_despertar(&mut fila, &mut espera, &mut estado, especial),
            Ok(true)
        );
        assert_eq!(
            inserir_e_despertar(&mut fila, &mut espera, &mut estado, mensagem(nova, 3)),
            Ok(false)
        );
        let recebida = preparar_recepcao(&mut fila, &mut espera, &mut estado, &atuais)
            .unwrap()
            .unwrap();
        assert_eq!(recebida.codigo, erro::CHAMADA);
        assert_eq!(recebida.a, 2);
        assert_eq!(retirar_atual(&mut fila, &atuais).unwrap().a, 3);
        assert!(fila.retirar().is_none());
    }
    #[test]
    fn recepcao_ignora_geracao_antiga_e_preserva_mensagens_atuais() {
        let antigas = referencia(REDE, 1);
        let novas = referencia(REDE, 2);
        let supervisor = referencia(SUPERVISOR, 1);
        let atuais = [referencia(TERMINAL, 1), novas, supervisor];
        let mut fila = CaixaMensagens::NOVA;
        for (origem, n) in [(antigas, 1), (supervisor, 2), (novas, 3)] {
            fila.inserir(mensagem(origem, n)).unwrap();
        }
        assert_eq!(retirar_atual(&mut fila, &atuais).unwrap().a, 2);
        assert_eq!(retirar_atual(&mut fila, &atuais).unwrap().a, 3);
        assert!(retirar_atual(&mut fila, &atuais).is_none());
    }
    #[test]
    fn admissao_desperta_somente_depois_de_inserir_e_preserva_fifo() {
        let mut fila = CaixaMensagens::NOVA;
        let mut estado = Estado::ATIVO;
        let mut espera = EsperaIpc::NOVA;
        let origem = referencia(TERMINAL, 1);
        assert_eq!(espera.registrar(&mut estado, 0), Ok(true));
        for n in 0..CAPACIDADE {
            fila.inserir(mensagem(origem, n as u64)).unwrap();
        }
        assert_eq!(
            inserir_e_despertar(&mut fila, &mut espera, &mut estado, mensagem(origem, 99)),
            Err(erro::FILA_CHEIA)
        );
        assert!(!estado.executavel());
        assert_eq!(fila.retirar().unwrap().a, 0);
        assert_eq!(
            inserir_e_despertar(&mut fila, &mut espera, &mut estado, mensagem(origem, 99)),
            Ok(true)
        );
        assert!(estado.executavel());
        for n in 1..CAPACIDADE {
            assert_eq!(fila.retirar().unwrap().a, n as u64);
        }
        assert_eq!(fila.retirar().unwrap().a, 99);
        assert_eq!(espera.registrar(&mut estado, 0), Ok(true));
        espera.cancelar();
        estado.terminar(crate::agendamento::Vida::Finalizada);
        let mut nova = Estado::ATIVO;
        assert_eq!(
            inserir_e_despertar(&mut fila, &mut espera, &mut nova, mensagem(origem, 100)),
            Ok(false)
        );
        assert!(nova.executavel());
    }
    #[test]
    fn recepcao_consume_mensagem_preexistente_e_acorda_uma_vez_apos_bloqueio() {
        let origem = referencia(TERMINAL, 1);
        let mut fila = CaixaMensagens::NOVA;
        let mut estado = Estado::ATIVO;
        let mut espera = EsperaIpc::NOVA;

        // Uma mensagem admitida antes da chamada de recepcao fica na FIFO;
        // nao ha espera pendente para acordar.
        assert_eq!(
            inserir_e_despertar(&mut fila, &mut espera, &mut estado, mensagem(origem, 1)),
            Ok(false)
        );
        assert!(estado.executavel());
        let atuais = [origem, referencia(REDE, 1), referencia(SUPERVISOR, 1)];
        assert_eq!(
            preparar_recepcao(&mut fila, &mut espera, &mut estado, &atuais)
                .unwrap()
                .unwrap()
                .a,
            1
        );
        assert!(estado.executavel());
        assert!(fila.retirar().is_none());

        // Com a caixa vazia, a mesma operacao registra a espera e bloqueia.
        assert!(
            preparar_recepcao(&mut fila, &mut espera, &mut estado, &atuais)
                .unwrap()
                .is_none()
        );
        assert!(!estado.executavel());
        assert_eq!(
            inserir_e_despertar(&mut fila, &mut espera, &mut estado, mensagem(origem, 2)),
            Ok(true)
        );
        assert!(estado.executavel());

        // O despertar e one-shot: uma segunda admissao permanece enfileirada,
        // sem gerar outro despertar para a mesma espera.
        assert_eq!(
            inserir_e_despertar(&mut fila, &mut espera, &mut estado, mensagem(origem, 3)),
            Ok(false)
        );
        assert_eq!(
            preparar_recepcao(&mut fila, &mut espera, &mut estado, &atuais)
                .unwrap()
                .unwrap()
                .a,
            2
        );
        assert_eq!(
            preparar_recepcao(&mut fila, &mut espera, &mut estado, &atuais)
                .unwrap()
                .unwrap()
                .a,
            3
        );
        assert!(fila.retirar().is_none());
    }
    #[test]
    fn fila_cheia_recusa_sem_substituir_mensagens() {
        let mut f = CaixaMensagens::NOVA;
        let origem = referencia(REDE, 1);
        for n in 0..CAPACIDADE {
            assert_eq!(f.inserir(mensagem(origem, n as u64)), Ok(()));
        }
        assert_eq!(f.inserir(mensagem(origem, 999)), Err(erro::FILA_CHEIA));
        for n in 0..CAPACIDADE {
            assert_eq!(f.retirar().unwrap().a, n as u64);
        }
        assert!(f.retirar().is_none());
    }
    #[test]
    fn quota_terminal_preserva_espaco_e_recupera_por_instancia_exata() {
        let antiga = referencia(REDE, 1);
        let nova = referencia(REDE, 2);
        let supervisor = referencia(SUPERVISOR, 1);
        let mut f = CaixaMensagens::NOVA;
        for n in 0..15 {
            assert_eq!(f.admitir_origem_terminal(antiga), Ok(()));
            f.inserir(mensagem(antiga, n)).unwrap();
        }
        assert_eq!(f.admitir_origem_terminal(nova), Err(erro::FILA_CHEIA));
        assert_eq!(f.admitir_origem_terminal(supervisor), Ok(()));
        f.inserir(mensagem(supervisor, 15)).unwrap();
        assert_eq!(f.inserir(mensagem(supervisor, 16)), Err(erro::FILA_CHEIA));
        assert_eq!(f.descartar_origem(nova), 0);
        assert_eq!(f.admitir_origem_terminal(nova), Err(erro::FILA_CHEIA));
        assert_eq!(f.retirar().unwrap().a, 0);
        assert_eq!(f.admitir_origem_terminal(nova), Ok(()));
        f.inserir(mensagem(nova, 16)).unwrap();
        assert_eq!(f.descartar_origem(antiga), 14);
        assert_eq!(f.retirar().unwrap().a, 15);
        assert_eq!(f.retirar().unwrap().a, 16);
        assert!(f.retirar().is_none());
    }
    #[test]
    fn quota_terminal_anel_envolvido_e_ambas_origens_sem_despertar_na_recusa() {
        for (principal, outra) in [(REDE, SUPERVISOR), (SUPERVISOR, REDE)] {
            let (a, b) = (referencia(principal, 1), referencia(outra, 1));
            let mut f = CaixaMensagens::NOVA;
            for n in 0..12 {
                f.inserir(mensagem(a, n)).unwrap();
            }
            for _ in 0..12 {
                f.retirar().unwrap();
            }
            let mut estado = Estado::ATIVO;
            let mut espera = EsperaIpc::NOVA;
            for n in 0..15 {
                inserir_terminal_e_despertar(&mut f, &mut espera, &mut estado, mensagem(a, n))
                    .unwrap();
            }
            assert_eq!(espera.registrar(&mut estado, 0), Ok(true));
            assert_eq!(
                inserir_terminal_e_despertar(&mut f, &mut espera, &mut estado, mensagem(a, 99)),
                Err(erro::FILA_CHEIA)
            );
            assert!(!estado.executavel());
            assert_eq!(
                inserir_terminal_e_despertar(&mut f, &mut espera, &mut estado, mensagem(b, 15)),
                Ok(true)
            );
            for n in 0..16 {
                assert_eq!(f.retirar().unwrap().a, n);
            }
            assert!(f.retirar().is_none());
        }
    }
    #[test]
    fn fila_preserva_fifo_apos_volta_do_anel() {
        let mut f = CaixaMensagens::NOVA;
        let origem = referencia(TERMINAL, 1);
        for n in 0..16 {
            f.inserir(mensagem(origem, n)).unwrap();
        }
        for n in 0..12 {
            assert_eq!(f.retirar().unwrap().a, n);
        }
        for n in 16..28 {
            f.inserir(mensagem(origem, n)).unwrap();
        }
        for n in 12..28 {
            assert_eq!(f.retirar().unwrap().a, n);
        }
        assert_eq!(f.tamanho(), 0);
    }
    #[test]
    fn purga_remove_apenas_origem_revogada_em_fila_circular() {
        let velha = referencia(REDE, 1);
        let nova = referencia(REDE, 2);
        let supervisor = referencia(SUPERVISOR, 1);
        let mut f = CaixaMensagens::NOVA;
        for n in 0..12 {
            f.inserir(mensagem(supervisor, n)).unwrap();
        }
        for _ in 0..10 {
            f.retirar();
        }
        for (origem, n) in [
            (velha, 12),
            (nova, 13),
            (velha, 14),
            (supervisor, 15),
            (velha, 16),
            (nova, 17),
        ] {
            f.inserir(mensagem(origem, n)).unwrap();
        }
        assert_eq!(f.descartar_origem(velha), 3);
        for n in [10, 11, 13, 15, 17] {
            assert_eq!(f.retirar().unwrap().a, n);
        }
        assert!(f.retirar().is_none());
    }
    #[cfg(feature = "teste-irq-fifo-cheia")]
    #[test]
    fn sonda_irq_fifo_cheia_confere_composicao_em_anel_e_ordem_das_origens() {
        let rede = referencia(REDE, 1);
        let supervisor = referencia(SUPERVISOR, 1);
        let mut fila = CaixaMensagens::NOVA;
        for n in 0..8 {
            fila.inserir(mensagem(rede, n as u64)).unwrap();
        }
        for _ in 0..8 {
            fila.retirar().unwrap();
        }
        for n in 1..=15u64 {
            fila.inserir(Mensagem {
                origem: rede,
                codigo: 0x7da,
                a: n,
                b: 0x11,
                c: 0x22,
                d: 0x33,
            })
            .unwrap();
        }
        fila.inserir(Mensagem {
            origem: supervisor,
            codigo: 0x7db,
            a: 1,
            b: 0x44,
            c: 0x55,
            d: 0x66,
        })
        .unwrap();
        assert!(fila.fila_irq_teste_confere(rede, supervisor, 15, true));

        let mut invertida = CaixaMensagens::NOVA;
        invertida
            .inserir(Mensagem {
                origem: supervisor,
                codigo: 0x7db,
                a: 1,
                b: 0x44,
                c: 0x55,
                d: 0x66,
            })
            .unwrap();
        for n in 1..=15u64 {
            invertida
                .inserir(Mensagem {
                    origem: rede,
                    codigo: 0x7da,
                    a: n,
                    b: 0x11,
                    c: 0x22,
                    d: 0x33,
                })
                .unwrap();
        }
        assert!(!invertida.fila_irq_teste_confere(rede, supervisor, 15, true));
        assert!(!fila.fila_irq_teste_confere(referencia(REDE, 2), supervisor, 15, true));
        assert!(!fila.fila_irq_teste_confere(rede, supervisor, 14, true));
    }
    #[test]
    fn limpar_remove_pedidos_destinados_instancia_antiga() {
        let mut f = CaixaMensagens::NOVA;
        for n in 0..16 {
            f.inserir(mensagem(referencia(TERMINAL, 1), n)).unwrap();
        }
        assert_eq!(f.limpar(), 16);
        assert!(f.retirar().is_none());
        f.inserir(mensagem(referencia(TERMINAL, 1), 100)).unwrap();
        assert_eq!(f.retirar().unwrap().a, 100);
        assert!(f.retirar().is_none());
    }
    #[test]
    fn validacao_recusa_referencias_de_outra_encarnacao() {
        let atuais = [
            referencia(TERMINAL, 1),
            referencia(REDE, 2),
            referencia(SUPERVISOR, 1),
        ];
        assert_eq!(
            localizar_instancia(referencia(REDE, 1), &atuais),
            Err(erro::REFERENCIA_ANTIGA)
        );
        assert_eq!(
            localizar_instancia(referencia(REDE, 3), &atuais),
            Err(erro::REFERENCIA_ANTIGA)
        );
        assert_eq!(localizar_instancia(referencia(REDE, 2), &atuais), Ok(1));
        assert_eq!(localizar_instancia(REDE, &atuais), Err(erro::SERVICO));
        assert_eq!(localizar_instancia(0, &atuais), Err(erro::SERVICO));
    }
    #[test]
    fn grafo_ipc_isola_supervisor_do_driver() {
        for origem in 0..=4 {
            for destino in 0..=4 {
                let esperado = matches!((origem, destino), (1, 2) | (2, 1) | (1, 3) | (3, 1));
                assert_eq!(pode_enviar(origem, destino), esperado);
            }
        }
    }
    #[test]
    fn arenas_sao_exclusivas_por_servico_e_encarnacao() {
        let referencias = [
            referencia(TERMINAL, 1),
            referencia(SUPERVISOR, 1),
            referencia(REDE, 1),
            referencia(REDE, 2),
            referencia(REDE, 3),
            referencia(REDE, 4),
        ];
        for (esperada, r) in referencias.into_iter().enumerate() {
            assert_eq!(arena_da_instancia(r), Ok(esperada));
        }
        assert_eq!(arena_da_instancia(referencia(REDE, 5)), Err(erro::RECURSOS));
        assert_eq!(
            arena_da_instancia(referencia(TERMINAL, 2)),
            Err(erro::REFERENCIA_ANTIGA)
        );
    }
    #[test]
    fn reinicio_nao_transborda_o_limite_de_encarnacoes() {
        for geracao in 1..MAX_INSTANCIAS_REDE {
            assert_eq!(
                proxima_instancia_rede(referencia(REDE, geracao)),
                Ok(referencia(REDE, geracao + 1))
            );
        }
        assert_eq!(
            proxima_instancia_rede(referencia(REDE, 4)),
            Err(erro::RECURSOS)
        );
        assert_eq!(
            proxima_instancia_rede(referencia(REDE, u32::MAX as u64)),
            Err(erro::RECURSOS)
        );
        assert_eq!(
            proxima_instancia_rede(referencia(TERMINAL, 1)),
            Err(erro::PERMISSAO)
        );
    }
    #[test]
    fn gerenciamento_exige_supervisor_e_referencia_atual_da_rede() {
        let atuais = [
            referencia(TERMINAL, 1),
            referencia(REDE, 2),
            referencia(SUPERVISOR, 1),
        ];
        for chamador in [TERMINAL, REDE] {
            assert_eq!(
                autorizar_gerenciamento(chamador, atuais[1], &atuais),
                Err(erro::PERMISSAO)
            );
            assert_eq!(
                autorizar_gerenciamento(chamador, 0, &atuais),
                Err(erro::PERMISSAO)
            );
        }
        assert_eq!(
            autorizar_gerenciamento(SUPERVISOR, atuais[0], &atuais),
            Err(erro::PERMISSAO)
        );
        assert_eq!(
            autorizar_gerenciamento(SUPERVISOR, referencia(REDE, 1), &atuais),
            Err(erro::REFERENCIA_ANTIGA)
        );
        assert_eq!(
            autorizar_gerenciamento(SUPERVISOR, atuais[1], &atuais),
            Ok(1)
        );
    }
}
