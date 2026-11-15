//! Filas limitadas, referencias de instancia e autorizacao estatica.
//! Nenhuma funcao deste modulo acessa hardware ou memoria global mutavel.
use contrato_sistema::{
    criar_referencia, erro, geracao_referencia, id_referencia, referencia_valida, Mensagem,
    MAX_INSTANCIAS_REDE, QUANTIDADE_SERVICOS, REDE, SUPERVISOR, TERMINAL,
};
pub const CAPACIDADE: usize = 16;
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
