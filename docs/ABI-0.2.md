# ABI 0.2 — identidade de instância e ciclo de vida

**Número no contrato binário:** `VERSAO_ABI = 2`.  
**Fonte normativa do projeto:** `contrato-sistema/src/lib.rs`.  
**Compatibilidade:** não há tradução ABI 1. As chamadas 11 a 17 são extensões aditivas opcionais da ABI 2; kernels ABI 2 anteriores a cada extensão retornam `CHAMADA`, e serviços novos precisam de fallback.

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
| 11 | `ESPERAR_IPC` | — | Zero para consultar a caixa própria; `ESTADO` em estado inválido; `CHAMADA` em kernel anterior |
| 12 | `RECEBER_BLOQUEANTE` | — | Status em `rax`: zero e mensagem, ou erro; `CHAMADA` em kernel anterior |
| 13 | `RECEBER_COM_PRAZO` | `rdi` modo, `rsi` duração | Status em `rax`: zero e mensagem, ou erro |
| 14 | `RECEBER_OBSERVANDO` | `rdi` referência exata, `rsi` modo, `rdx` duração | Status em `rax`: zero e mensagem, ou erro; supervisor observa rede |
| 15 | `INSCREVER_VIDA` | `rdi` referência exata | `rax=0`, `rdi` estado inicial, ou erro |
| 16 | `CANCELAR_VIDA` | `rdi` referência inscrita | Zero ou erro |
| 17 | `ESPERAR_EVENTO` | `rdi` modo, `rsi` duração | `rax=1` IPC, `rax=2` vida, ou erro |
| 18 | `ARMAR_IRQ4` | — | Zero ou erro; exclusivo do terminal |
| 19 | `CONFIRMAR_IRQ4` | `rdi` sequência | Zero ou erro; exclusivo do terminal |
| 20 | `ESPERAR_IRQ_IPC` | `rdi` modo, `rsi` duração | `rax=1` IPC, `rax=3` IRQ com sequência em `rdi`, ou erro |

Estas chamadas opcionais mantêm ABI 2 e retornam `CHAMADA` em kernels anteriores.
IRQ4 pertence somente à referência atual do terminal; outros serviços recebem
`PERMISSAO`. Armar novamente ou confirmar sem entrega, com sequência diferente
ou repetida retorna `ESTADO` sem desmascarar. A notificação é um slot coalescido
separado da FIFO, não uma contagem de bytes; IPC e IRQ alternam quando ambos estão
disponíveis. A sequência não é reutilizada; esgotamento retorna `RECURSOS` com a
linha mascarada. Prazo e erros seguem `RECEBER_COM_PRAZO`; o prazo vencido tem
precedência e não consome a pendência. Ao terminar ou falhar, a associação é
cancelada e a linha permanece mascarada.

O terminal configura e atende a UART antes de armar, drena as causas antes de
confirmar e verifica novamente o dispositivo depois da confirmação e antes de
dormir. Se houver trabalho, continua localmente sem aguardar outra borda. O
kernel controla somente PIC e estado da notificação; não interpreta a UART.
Esta extensão não habilita IRQ4 automaticamente nem oferece IRQ compartilhada.
Detalhes e limites estão na [ADR 010](ADR-010-NOTIFICACAO-IRQ4.md).

O wrapper `apoio_servicos::estado(id)` é uma conveniência que resolve o ID antes
da consulta. `estado_referencia(referencia)` permite consultar uma instância
exata e verificar rejeição de referências antigas. `enviar` nunca resolve
automaticamente a referência fornecida pelo chamador.

`ESPERAR_IPC` opera exclusivamente na caixa do próprio chamador. Com fila não
vazia retorna imediatamente; caso contrário registra espera e bloqueia até
um envio autorizado ser admitido. Verificação e registro são atômicos perante
envios na CPU única com interrupções desabilitadas. Envio recusado não acorda o
destino. A mensagem continua na FIFO até `RECEBER`, que permanece não bloqueante.
Zero sinaliza para consultar a caixa, não garante que uma mensagem ainda esteja
presente: uma revogação pode purgá-la antes da retomada. Se `RECEBER` retornar
vazio, o serviço pode rearmar a espera. Não há timeout, IRQ ou cancelamento remoto.
Em kernel antigo, `erro::CHAMADA` indica ausência da extensão e o serviço deve
continuar sua política anterior, por exemplo ceder e consultar novamente. Outros
erros não devem ser tratados como mensagens. O supervisor registra erro
inesperado uma vez e volta a ceder. A combinação de serviço novo e kernel antigo
ainda não foi ensaiada em VM. Veja [ADR 004](ADR-004-ESPERA-IPC-ABI-2.md).

`RECEBER_BLOQUEANTE` recebe a primeira mensagem válida da própria caixa ou
bloqueia até ela existir. Em sucesso, `rax=0`, `rdi=origem`, `rsi=codigo`,
`rdx=a`, `r10=b`, `r8=c`, `r9=d`; em falha, `rax` contém o erro e os outros
resultados não são significativos. A separação de status permite inclusive que
o código de mensagem seja igual a um código de erro. A consulta e a publicação
da espera são atômicas perante envios na CPU única, com IF=0. Após despertar, o
kernel consome a FIFO quando seleciona o processo; uma purga anterior pode
esvaziá-la e provocar novo bloqueio sem retorno falso. A mensagem é considerada
consumida na seleção mesmo se uma IRQ ocorrer antes da primeira instrução CPL 3.
`ESTADO` indica estado inválido; `CHAMADA` em kernel anterior permite fallback
para espera de disponibilidade ou polling. Não há prazo nem cancelamento remoto.
Veja [ADR 005](ADR-005-RECEPCAO-BLOQUEANTE.md).

`RECEBER_COM_PRAZO` usa o mesmo layout de saída da chamada 12. Os modos são
`IMEDIATO=0`, `FINITO=1` e `SEM_PRAZO=2`; a duração deve ser zero nos modos
imediato e sem prazo, e de 1 a 2^63−1 tiques no finito. Outros campos de modo
ou duração retornam `CHAMADA` antes de consumir a fila. Tentativa imediata
retorna `AUSENTE` se vazia; prazo finito esgotado retorna `TEMPO`. Um kernel
anterior também retorna `CHAMADA` para o número desconhecido; o serviço deve
tratar essa ausência de extensão com uma política compatível. O desempate e a
aritmética modular estão na [ADR 006](ADR-006-PRAZOS-IPC.md).

`RECEBER_OBSERVANDO` espera mensagem na própria caixa ou indisponibilidade da
instância exata da rede. Somente o supervisor é autorizado; autoridade é verificada
antes de consultar o alvo. Usa modos, duração e saída de mensagem da chamada 13.
Referência antiga retorna `REFERENCIA_ANTIGA`, alvo já finalizado ou falho retorna
`ALVO_INDISPONIVEL`, e alvo inválido retorna `SERVICO`. Enquanto a espera estiver
pendente, falha, saída, encerramento ou substituição do alvo concluem com
`ALVO_INDISPONIVEL` sem consumir a FIFO. Prazo vencido prevalece; mensagem já
entregue permanece consumida. Tentativa imediata com alvo ativo e caixa vazia
retorna `AUSENTE`. Essa observação vale apenas durante a chamada, sem inscrição
ou notificação persistente. Uma instância que morre não retorna de sua espera.
Falha não revoga a referência: mensagens da geração falha podem permanecer na
fila até serem consumidas ou purgadas em encerramento/reinício autorizado.
Veja [ADR 007](ADR-007-ESPERA-E-CICLO-DE-VIDA.md).

`INSCREVER_VIDA` registra um slot reservado para o supervisor observar a
referência exata da rede e devolve o estado inicial na mesma região crítica, sem
duplicar uma notificação. Outra inscrição para a mesma referência retorna
`ESTADO`; uma referência diferente enquanto o slot estiver ocupado retorna
`RECURSOS`. Referência antiga é recusada na inscrição. `CANCELAR_VIDA` exige a
mesma autorização e a referência inscrita, mesmo quando já não for a atual;
slot vazio retorna `ESTADO`, referência diferente `REFERENCIA_ANTIGA`.

`ESPERAR_EVENTO` exige inscrição ativa e usa os modos e erros de prazo da chamada
13. Em IPC, `rax=1` e os seis registradores seguem o layout da chamada 12. Em
vida, `rax=2`, `rdi=referência`, `rsi=tipo` (`1` estado ou `2` invalidação),
`rdx=estado`; os demais valores são zero. Classe do kernel é independente do
código da mensagem. Mudanças coalescem o estado mais recente; a invalidação é
terminal, conserva o último estado observado antes da troca e impede nova
inscrição até consumo ou cancelamento. Em reinício de instância ativa esse
estado é executável; se já havia falhado, é falhou. Prazo vencido prevalece sem
consumir nenhuma fonte; entre IPC e vida elegíveis, as classes alternam. Veja
[ADR 008](ADR-008-NOTIFICACOES-VIDA.md).

## 3. Mensagens e autoridade

`Mensagem` permanece com seis palavras de 64 bits: `origem`, `codigo`, `a`, `b`,
`c`, `d`. A mudança é que `origem` contém a referência completa, preenchida pelo
kernel. O remetente não fornece essa identidade na syscall.

No retorno de `RECEBER`, `rax=codigo`, `rdi=origem`, `rsi=a`, `rdx=b`, `r10=c`,
`r8=d`, `r9=0`. Cada caixa comporta 16 mensagens. Na caixa do terminal,
rede e supervisor podem ocupar até 15 posições cada; o limite conta pelo ID
lógico, inclusive gerações ainda presentes. Superar esse limite ou encher a
caixa retorna `FILA_CHEIA` sem substituir mensagens ou despertar uma espera.
Retirada e purga liberam espaço; a purga identifica a referência exata.
As demais caixas mantêm capacidade de 16 para sua única origem autorizada.

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
| `FILA_CHEIA` | `u64::MAX - 2` | Caixa de destino cheia ou quota da origem na caixa do terminal atingida |
| `SERVICO` | `u64::MAX - 3` | Destino inexistente ou indisponível |
| `CHAMADA` | `u64::MAX - 4` | Operação ou argumento inválido |
| `REFERENCIA_ANTIGA` | `u64::MAX - 5` | Geração não corresponde à instância atual |
| `RECURSOS` | `u64::MAX - 6` | Arena de encarnação indisponível, slot de observação ocupado ou sequência IRQ esgotada |
| `ESTADO` | `u64::MAX - 7` | Estado incompatível com a operação |
| `IMAGEM` | `u64::MAX - 8` | Falha de preparação da nova imagem |
| `TEMPO` | `u64::MAX - 9` | Prazo finito de espera esgotado |
| `AUSENTE` | `u64::MAX - 10` | Tentativa imediata sem mensagem válida |
| `ALVO_INDISPONIVEL` | `u64::MAX - 11` | Instância observada falhou, terminou ou foi substituída |

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
