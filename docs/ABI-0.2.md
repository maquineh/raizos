# ABI 0.2 — identidade de instância e ciclo de vida

**Número no contrato binário:** `VERSAO_ABI = 2`.  
**Fonte normativa do projeto:** `contrato-sistema/src/lib.rs`.  
**Compatibilidade:** reconstruir kernel e todos os serviços; não há tradução ABI 1.

## 1. Identidades e referências

| Identidade lógica | Serviço | Geração inicial |
|---:|---|---:|
| 1 | Terminal | 1 |
| 2 | Rede | 1 |
| 3 | Supervisor | 1 |

Uma referência é um `u64` com a geração nos 32 bits superiores e o identificador
nos 32 inferiores. Zero não é geração válida. Exemplo: a primeira rede é
`0x0000000100000002`; após um reinício é `0x0000000200000002`.

`criar_referencia`, `id_referencia`, `geracao_referencia` e `referencia_valida`
definem a codificação. O conhecimento de uma referência não concede direitos:
o kernel verifica a identidade real do chamador e a política estática.

Somente a rede é reiniciável nesta etapa, com quatro encarnações no máximo por
boot. Terminal e supervisor não mudam de geração na implementação atual.

## 2. Convenção de chamada

O serviço executa `int 0x80`. Número em `rax`; argumentos em
`rdi`, `rsi`, `rdx`, `r10`, `r8`, `r9`. O resultado fica em `rax`.
`RECEBER` também preenche os registradores de mensagem abaixo.

Nenhum ponteiro Rust ou endereço de buffer é passado pelo IPC desta versão.

`PCI_COMANDO` aceita somente `0x0402` ou `0x0406`: ambos mantêm MMIO ativo e
INTx desabilitado; o segundo habilita bus mastering. As demais combinações são
recusadas, mesmo quando o chamador é o serviço de rede.

| Número | Chamada | Entrada | Resultado |
|---:|---|---|---|
| 0 | `CEDER` | — | Zero; permite reagendamento |
| 1 | `ENVIAR` | Referência destino, código, quatro valores | Zero ou erro |
| 2 | `RECEBER` | — | Código e valores; código zero significa fila vazia |
| 3 | `SAIR` | — | Finaliza o próprio serviço; não retorna |
| 4 | `PCI_LER` | BDF e registro | Valor ou erro; restrito à rede |
| 5 | `PCI_COMANDO` | BDF e valor `0x0402` ou `0x0406` | Zero ou erro; restrito à rede; demais valores retornam `PERMISSAO` |
| 6 | `ESTADO_SERVICO` | Referência exata | Estado ou erro de referência |
| 7 | `TIQUES` | — | Contador do timer |
| 8 | `RESOLVER` | Identificador lógico | Referência atual, inclusive falha/finalizada, ou erro |
| 9 | `REINICIAR` | Referência exata da rede | Nova referência ou erro; restrito ao supervisor |
| 10 | `ENCERRAR` | Referência exata da rede | Zero ou erro; restrito ao supervisor |

O wrapper `apoio_servicos::estado(id)` é uma conveniência que resolve o ID antes
da consulta. `estado_referencia(referencia)` permite consultar uma instância
exata e verificar rejeição de referências antigas. `enviar` nunca resolve
automaticamente a referência fornecida pelo chamador.

## 3. Mensagens e autoridade

`Mensagem` permanece com seis palavras de 64 bits: `origem`, `codigo`, `a`, `b`,
`c`, `d`. A mudança é que `origem` contém a referência completa, preenchida pelo
kernel. O remetente não fornece essa identidade na syscall.

No retorno de `RECEBER`, `rax=codigo`, `rdi=origem`, `rsi=a`, `rdx=b`, `r10=c`,
`r8=d`, `r9=0`. A caixa comporta 16 mensagens.

As únicas arestas permitidas são terminal→rede, rede→terminal,
terminal→supervisor e supervisor→terminal. Não há autoenvio. O código zero é
reservado à ausência de mensagem.

Ao revogar uma instância, sua caixa é esvaziada e suas mensagens pendentes são
removidas das caixas dos demais processos. Mensagens já consumidas não podem ser
desfeitas; o cliente deve considerar a referência que identificava seu remetente.

## 4. Estados e erros

Estados: 0 inexistente, 1 executável, 2 falhou, 3 finalizado. Uma referência antiga
não deve ser confundida com o estado da nova instância que reutiliza o ID lógico.

`ENCERRAR` exige estado executável; falhou/finalizado retorna `ESTADO` sem mudar
a geração. `REINICIAR` aceita esses três estados, desde que a referência seja
atual e exista orçamento para uma nova encarnação.

| Constante | Valor | Significado |
|---|---|---|
| `PERMISSAO` | `u64::MAX - 1` | Operação sem autoridade |
| `FILA_CHEIA` | `u64::MAX - 2` | Caixa de destino sem espaço |
| `SERVICO` | `u64::MAX - 3` | Destino inexistente ou indisponível |
| `CHAMADA` | `u64::MAX - 4` | Operação ou argumento inválido |
| `REFERENCIA_ANTIGA` | `u64::MAX - 5` | Geração não corresponde à instância atual |
| `RECURSOS` | `u64::MAX - 6` | Não há arena disponível para outra encarnação |
| `ESTADO` | `u64::MAX - 7` | Estado incompatível com a operação |
| `IMAGEM` | `u64::MAX - 8` | Falha de preparação da nova imagem |

A verificação de autoridade de reinício/encerramento ocorre antes de modificar o
alvo. Esgotamento de recursos não encerra uma instância ainda executável.

## 5. Novos eventos de supervisor

Os códigos 1 a 10 de rede mantêm o significado da ABI anterior; suas origens e
destinos passam a utilizar referências de instância.

| Código | Evento | Valores |
|---:|---|---|
| 11 | `SUPERVISOR_PRONTO` | a=modo, b=limite de encarnações, c=CPL |
| 12 | `SUPERVISAO_CONFIGURAR` | a=0 manual ou 1 automático |
| 13 | `SUPERVISAO_ESTADO` | a=modo, b=reinícios realizados, c=último erro, d=limite |
| 14 | `SERVICO_REINICIAR` | a=referência alvo |
| 15 | `SERVICO_REINICIADO` | a=antiga, b=nova, c=0 manual ou 1 automático |
| 16 | `SERVICO_ENCERRAR` | a=referência alvo |
| 17 | `SERVICO_ENCERRADO` | a=referência encerrada |
| 18 | `SUPERVISOR_ERRO` | a=erro, b=operação, c=referência relacionada |
| 19 | `SUPERVISAO_CONSULTAR` | Sem argumentos |

Somente o terminal solicita mudanças ao supervisor. O retorno bem-sucedido da
syscall `REINICIAR` confirma a criação do novo contexto; a notificação posterior
`REDE_PRONTA` confirma que a inicialização do driver chegou ao fim.

## 6. Informações de entrada e memória

`InfoServico` acrescenta `referencia: u64` imediatamente após `id`. A estrutura
tem 104 bytes, alinhamento 8 e `referencia` no deslocamento 24. Assinatura,
versão, tamanho e ordenação devem ser os declarados no crate de contrato.
A página continua somente leitura para o processo.

As janelas virtuais de código, pilha, informações, framebuffer e MMIO continuam
iguais às da ABI 0.1. Cada encarnação da rede recebe uma nova reserva física de
2 MiB, mantendo os mesmos endereços virtuais. As reservas anteriores ficam em
quarentena até o reboot.

Essa política não fornece memória dinâmica de propósito geral ou isolamento de
DMA por IOMMU. Veja [ADR 002](ADR-002-CICLO-DE-VIDA.md).
