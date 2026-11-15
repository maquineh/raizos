#![no_std]
//! Primitivas de rede próprias, sem alocação ou dependência de um SO hospedeiro.
//!
//! Escopo: IPv4 sem fragmentação/opções e UDP; este crate não é uma pilha IP
//! completa. As funções de leitura nunca confiam nos comprimentos da rede.
//! Referências: RFC 791 (IPv4), RFC 768 (UDP) e RFC 1071 (checksum).

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ErroPacote {
    Truncado,
    Versao,
    OpcoesNaoSuportadas,
    Comprimento,
    Checksum,
    Fragmentado,
    Reservado,
    TempoEsgotado,
    Protocolo,
}

pub fn ler_u16(bytes: &[u8], posicao: usize) -> u16 {
    u16::from_be_bytes([bytes[posicao], bytes[posicao + 1]])
}

pub fn escrever_u16(bytes: &mut [u8], posicao: usize, valor: u16) {
    bytes[posicao..posicao + 2].copy_from_slice(&valor.to_be_bytes());
}

fn somar(mut soma: u32, dados: &[u8]) -> u32 {
    let mut pares = dados.chunks_exact(2);
    for par in &mut pares {
        soma += u16::from_be_bytes([par[0], par[1]]) as u32;
        // Reduzir a cada par evita overflow mesmo em fatias muito grandes.
        soma = (soma & 0xffff) + (soma >> 16);
    }
    if let Some(&ultimo) = pares.remainder().first() {
        soma += (ultimo as u32) << 8;
    }
    soma
}

fn concluir(mut soma: u32) -> u16 {
    while soma >> 16 != 0 {
        soma = (soma & 0xffff) + (soma >> 16);
    }
    !(soma as u16)
}

/// Soma de complemento de um; retorna zero para uma mensagem já válida.
pub fn checksum(dados: &[u8]) -> u16 {
    concluir(somar(0, dados))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PacoteIpv4<'a> {
    pub origem: [u8; 4],
    pub destino: [u8; 4],
    pub protocolo: u8,
    pub carga: &'a [u8],
}

/// Aceita apenas um datagrama IPv4 não fragmentado e sem opções.
pub fn ler_ipv4(bytes: &[u8]) -> Result<PacoteIpv4<'_>, ErroPacote> {
    if bytes.len() < 20 {
        return Err(ErroPacote::Truncado);
    }
    if bytes[0] >> 4 != 4 {
        return Err(ErroPacote::Versao);
    }
    if bytes[0] & 0x0f != 5 {
        return Err(ErroPacote::OpcoesNaoSuportadas);
    }
    let tamanho = ler_u16(bytes, 2) as usize;
    if tamanho < 20 || tamanho > bytes.len() {
        return Err(ErroPacote::Comprimento);
    }
    let fragmentacao = ler_u16(bytes, 6);
    if fragmentacao & 0x8000 != 0 {
        return Err(ErroPacote::Reservado);
    }
    if fragmentacao & 0x3fff != 0 {
        return Err(ErroPacote::Fragmentado);
    }
    if bytes[8] == 0 {
        return Err(ErroPacote::TempoEsgotado);
    }
    if checksum(&bytes[..20]) != 0 {
        return Err(ErroPacote::Checksum);
    }
    Ok(PacoteIpv4 {
        origem: [bytes[12], bytes[13], bytes[14], bytes[15]],
        destino: [bytes[16], bytes[17], bytes[18], bytes[19]],
        protocolo: bytes[9],
        carga: &bytes[20..tamanho],
    })
}

pub fn montar_ipv4(
    saida: &mut [u8],
    origem: [u8; 4],
    destino: [u8; 4],
    protocolo: u8,
    identificador: u16,
    carga: &[u8],
) -> Result<usize, ErroPacote> {
    let tamanho = 20usize
        .checked_add(carga.len())
        .ok_or(ErroPacote::Comprimento)?;
    if tamanho > u16::MAX as usize || tamanho > saida.len() {
        return Err(ErroPacote::Comprimento);
    }
    saida[..20].fill(0);
    saida[0] = 0x45;
    escrever_u16(saida, 2, tamanho as u16);
    escrever_u16(saida, 4, identificador);
    escrever_u16(saida, 6, 0x4000); // DF; não implementamos fragmentação.
    saida[8] = 64;
    saida[9] = protocolo;
    saida[12..16].copy_from_slice(&origem);
    saida[16..20].copy_from_slice(&destino);
    let soma = checksum(&saida[..20]);
    escrever_u16(saida, 10, soma);
    saida[20..tamanho].copy_from_slice(carga);
    Ok(tamanho)
}

/// Checksum com pseudo-cabeçalho IPv4. O datagrama deve ter no máximo 65535 bytes.
pub fn checksum_udp(origem: [u8; 4], destino: [u8; 4], datagrama: &[u8]) -> u16 {
    let pseudo = [
        origem[0],
        origem[1],
        origem[2],
        origem[3],
        destino[0],
        destino[1],
        destino[2],
        destino[3],
        0,
        17,
        (datagrama.len() >> 8) as u8,
        datagrama.len() as u8,
    ];
    concluir(somar(somar(0, &pseudo), datagrama))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DatagramaUdp<'a> {
    pub porta_origem: u16,
    pub porta_destino: u16,
    pub carga: &'a [u8],
}

pub fn ler_udp<'a>(ip: &PacoteIpv4<'a>) -> Result<DatagramaUdp<'a>, ErroPacote> {
    if ip.protocolo != 17 {
        return Err(ErroPacote::Protocolo);
    }
    if ip.carga.len() < 8 {
        return Err(ErroPacote::Truncado);
    }
    let tamanho = ler_u16(ip.carga, 4) as usize;
    if tamanho < 8 || tamanho != ip.carga.len() {
        return Err(ErroPacote::Comprimento);
    }
    // Em IPv4, checksum UDP zero significa que o emissor não o calculou.
    if ler_u16(ip.carga, 6) != 0 && checksum_udp(ip.origem, ip.destino, ip.carga) != 0 {
        return Err(ErroPacote::Checksum);
    }
    Ok(DatagramaUdp {
        porta_origem: ler_u16(ip.carga, 0),
        porta_destino: ler_u16(ip.carga, 2),
        carga: &ip.carga[8..tamanho],
    })
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn checksum_vetor_conhecido_e_comprimento_impar() {
        assert_eq!(
            checksum(&[0x00, 0x01, 0xf2, 0x03, 0xf4, 0xf5, 0xf6, 0xf7]),
            0x220d
        );
        assert_eq!(checksum(&[1, 2, 3]), 0xfbfd);
        assert_eq!(checksum(&[]), 0xffff);
    }

    #[test]
    fn ipv4_remove_padding_ethernet() {
        let mut pacote = [0u8; 64];
        let tamanho =
            montar_ipv4(&mut pacote, [10, 0, 2, 2], [10, 0, 2, 15], 1, 7, b"abc").unwrap();
        assert_eq!(tamanho, 23);
        let lido = ler_ipv4(&pacote).unwrap();
        assert_eq!(lido.carga, b"abc");
        assert_eq!(lido.origem, [10, 0, 2, 2]);
    }

    #[test]
    fn ipv4_rejeita_truncamento_em_todos_os_pontos() {
        let mut pacote = [0u8; 24];
        montar_ipv4(&mut pacote, [1, 2, 3, 4], [5, 6, 7, 8], 1, 9, b"1234").unwrap();
        for comprimento in 0..24 {
            assert!(ler_ipv4(&pacote[..comprimento]).is_err());
        }
    }

    #[test]
    fn ipv4_rejeita_checksum_incorreto() {
        let mut pacote = [0u8; 20];
        montar_ipv4(&mut pacote, [1, 2, 3, 4], [5, 6, 7, 8], 1, 9, b"").unwrap();
        pacote[12] ^= 1;
        assert_eq!(ler_ipv4(&pacote), Err(ErroPacote::Checksum));
    }

    #[test]
    fn ipv4_rejeita_fragmentos_e_opcoes() {
        let mut pacote = [0u8; 20];
        montar_ipv4(&mut pacote, [1, 2, 3, 4], [5, 6, 7, 8], 1, 9, b"").unwrap();
        pacote[6] |= 0x20;
        assert_eq!(ler_ipv4(&pacote), Err(ErroPacote::Fragmentado));
        pacote[6] = 0x40;
        pacote[0] = 0x46;
        assert_eq!(ler_ipv4(&pacote), Err(ErroPacote::OpcoesNaoSuportadas));
    }

    #[test]
    fn udp_verifica_checksum_incluindo_origem() {
        let origem = [10, 0, 2, 2];
        let destino = [10, 0, 2, 15];
        let mut udp = [0u8; 11];
        escrever_u16(&mut udp, 0, 1234);
        escrever_u16(&mut udp, 2, 7777);
        escrever_u16(&mut udp, 4, 11);
        udp[8..].copy_from_slice(b"ola");
        let soma = checksum_udp(origem, destino, &udp);
        escrever_u16(&mut udp, 6, if soma == 0 { 0xffff } else { soma });
        let ip = PacoteIpv4 {
            origem,
            destino,
            protocolo: 17,
            carga: &udp,
        };
        assert_eq!(ler_udp(&ip).unwrap().carga, b"ola");
        let adulterado = PacoteIpv4 {
            origem: [10, 0, 2, 3],
            ..ip
        };
        assert_eq!(ler_udp(&adulterado), Err(ErroPacote::Checksum));
    }

    #[test]
    fn udp_aceita_checksum_zero_em_ipv4() {
        let udp = [0, 1, 0x1e, 0x61, 0, 8, 0, 0];
        let ip = PacoteIpv4 {
            origem: [1, 2, 3, 4],
            destino: [5, 6, 7, 8],
            protocolo: 17,
            carga: &udp,
        };
        assert_eq!(ler_udp(&ip).unwrap().porta_destino, 7777);
    }

    #[test]
    fn udp_rejeita_comprimento_falso() {
        let udp = [0, 1, 0x1e, 0x61, 0, 7, 0, 0];
        let ip = PacoteIpv4 {
            origem: [1, 2, 3, 4],
            destino: [5, 6, 7, 8],
            protocolo: 17,
            carga: &udp,
        };
        assert_eq!(ler_udp(&ip), Err(ErroPacote::Comprimento));
    }

    #[test]
    fn saida_curta_e_recusada_sem_escrever() {
        let mut saida = [0xa5u8; 20];
        assert_eq!(
            montar_ipv4(&mut saida, [0; 4], [0; 4], 1, 0, b"x"),
            Err(ErroPacote::Comprimento)
        );
        assert_eq!(saida, [0xa5; 20]);
    }
}
