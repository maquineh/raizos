# RZ-025 — evidências parciais do marco 0.3

Este registro acrescenta uma evidência de integração à matriz T03; **RZ-025
permanece em andamento** e o marco 0.3 não está concluído.

## Preparação recusada antes do commit

O ensaio QEMU [teste-falha-preparacao-1794743041965761600](../teste-falha-preparacao-1794743041965761600/resultado.txt)
foi aprovado; consulte também o [serial](../teste-falha-preparacao-1794743041965761600/serial.log).
O supervisor autorizado acionou o caminho real de reinício; a fixture forneceu
um ELF vazio/inválido à preparação da nova arena. O loader rejeitou o ELF antes
da revogação/commit, o erro `IMAGEM` retornou, e a instância antiga permaneceu
na geração 1 com referência `0x100000002`. O serial confirma `REINICIO_RECUSADO`,
`OPERACAO_RECUSADA` e estado `rede=1 geracao_rede=1`. A rede respondeu ao ping
antes e depois da recusa; o resultado registra também quatro ecos UDP após a
recusa.

Esta fixture valida falha na validação inicial do ELF. Não representa exaustão
física, falha de alocação/página ou todas as classes de erro do loader; tampouco
prova recuperação de conexões ou estado de aplicações.

A tentativa anterior [teste-falha-preparacao-1794742892087494800](../teste-falha-preparacao-1794742892087494800/resultado.txt)
falhou aguardando o marcador porque o parser não aceitava o prompt que o
precedia. Ela permanece preservada; o parser foi corrigido e a nova execução
aprovou. Os resultados anteriores não foram substituídos.

## Validação e rastreabilidade

A validação normal está em [falha-preparacao-validacao](falha-preparacao-validacao/):
cinco boots (smoke, isolamento e recuperação), `cargo fmt --all --check`, 38
testes de pacotes (3 + 26 + 9), 34 testes do núcleo, build e verificação de
imagem. Os manifests de [fontes](falha-preparacao-validacao/fontes.sha256) e
[artefatos](falha-preparacao-validacao/artefatos.sha256) relacionam essa
validação; os relatórios, seriais e logs de comando estão na pasta.

Os `qemu-*-stderr.log` copiados com os relatórios dos boots normais têm data
histórica de 09/11/2026 e não são atribuíveis a essa validação. O stderr do
ensaio opt-in atual permanece no diretório do próprio teste.

T03-10 tem evidência em VM para esta classe delimitada de recusa de preparação;
a cobertura de RZ-025 permanece parcial, conforme a [matriz T03](../execucao-rz-023-20261115/MATRIZ.md).

## IRQ4 com FIFO IPC cheia — T03-11

As execuções QEMU [1794744323818426700](../teste-irq-fifo-cheia-1794744323818426700/resultado.txt)
e [1794744327832498300](../teste-irq-fifo-cheia-1794744327832498300/resultado.txt)
foram aprovadas. Após endurecer o parser do oráculo, a repetição com o oráculo
final [1794744539244129100](../teste-irq-fifo-cheia-1794744539244129100/resultado.txt)
também foi aprovada. Os [seriais da primeira](../teste-irq-fifo-cheia-1794744323818426700/serial.log),
[segunda](../teste-irq-fifo-cheia-1794744327832498300/serial.log) e
[repetição final](../teste-irq-fifo-cheia-1794744539244129100/serial.log)
registram o mesmo cenário: 15 IPCs da rede mais uma do supervisor ocupam a FIFO
de 16 entradas; após um byte UART real, IRQ4 permanece pendente/selecionável e
mascarada no PIC enquanto a FIFO está cheia. O seletor escolhe primeiro uma IPC,
depois a IRQ, e então as 15 IPCs restantes. O terminal confere origem, código,
payload e ordem; a sonda final confirma FIFO vazia.

O resultado cobre essa composição finita em QEMU. Não demonstra todas as
intercalações possíveis, justiça sob fluxo ilimitado, resistência a tempestade
UART nem funcionamento em hardware físico. As execuções anteriores, incluindo
as duas aprovações anteriores [1794744277467154800](../teste-irq-fifo-cheia-1794744277467154800/resultado.txt)
e [1794744290122562700](../teste-irq-fifo-cheia-1794744290122562700/resultado.txt),
as duas pós-isolamento e a repetição com parser final, permanecem preservadas
em seus diretórios próprios.

O cenário de kernel é implementado em [`nucleo/src/irq.rs`](../../nucleo/src/irq.rs#L36);
o comando e seu oráculo estão em [`ferramentas/src/main.rs`](../../ferramentas/src/main.rs#L4445).

A rastreabilidade desta etapa está em [irq-fifo-validacao](irq-fifo-validacao/):
os manifests [de fontes finais](irq-fifo-validacao/fontes-oraculo-final.sha256)
e [de artefatos finais](irq-fifo-validacao/artefatos-oraculo-final.sha256),
[testes host do parser final](irq-fifo-validacao/testes-ferramentas-oraculo-final.log),
e logs/relatórios dos cinco boots normais. Entre os manifests de fontes anterior
e final, somente `ferramentas/src/main.rs` mudou; os novos hashes identificam a
fonte, ferramenta e imagem usados no oráculo final. Os 39 testes de pacotes, 34
testes de núcleo e 35 testes de núcleo com feature estão nos logs; os 35 incluem
os mesmos 34 testes, portanto não são uma contagem adicional. `cargo fmt
--all --check` também passou para essa validação.

Os `qemu-*-stderr.log` copiados junto dos relatórios dos boots normais são
históricos de 09/11/2026 e não podem ser atribuídos a estas novas execuções. Os
seriais e resultados QEMU de T03-11 estão nos diretórios individuais dos ensaios.
RZ-025 continua em andamento e o marco 0.3 não está concluído; veja a [matriz
T03](../execucao-rz-023-20261115/MATRIZ.md) e o status no backlog.

## Recepção UART por fase — T03-16

As execuções finais em QEMU [1794746072846004000](../teste-uart-rz025-1794746072846004000/resultado.txt)
e [1794746077826314400](../teste-uart-rz025-1794746077826314400/resultado.txt)
foram aprovadas. Em cada uma, o host submeteu `0x55` na fase UNICO e os bytes
`0x31..0x38` na fase RAJADA; os [seriais da primeira](../teste-uart-rz025-1794746072846004000/serial.log)
e [segunda](../teste-uart-rz025-1794746077826314400/serial.log) registram em
ambas as fases uma IRQ4 do kernel, amostra IIR bruto `0xcc`, bit 0 do IIR em
`0` (indicando interrupção pendente), causa timeout RX `0x0c` e `rbr_antes=0`.
O terminal leu pelo RBR um byte na
primeira fase e oito na segunda, com IDs em ordem e valores iguais aos
submetidos. Ambas registraram `lsr_oe=0`; isso significa que não foi observado
o indicador OE do LSR nessas janelas.

As execuções anteriores [1794745626449479100](../teste-uart-rz025-1794745626449479100/resultado.txt)
e [1794745669826929700](../teste-uart-rz025-1794745669826929700/resultado.txt)
usaram o parser anterior e permanecem preservadas. A auditoria identificou que
ele não validava o IIR bruto/pending e o pareamento da causa com a leitura RBR;
o oráculo foi reforçado antes dos dois resultados finais. O parser atual exige
bit 0 do IIR zerado, indicando IRQ pendente, causa RX permitida coerente com o IIR bruto e posição
`rbr_antes` correspondente ao próximo RBR observado. O código do [oráculo e
harness](../../ferramentas/src/main.rs#L811), da [instrumentação de recepção e
IRQ](../../servicos/terminal/src/main.rs#L1130) e do [acesso IIR/LSR](../../servicos/terminal/src/io.rs#L34)
está ligado aos registros da execução.

O [estímulo da primeira execução](../teste-uart-rz025-1794746072846004000/estimulo-host.log)
e [da segunda](../teste-uart-rz025-1794746077826314400/estimulo-host.log)
documenta bytes escritos no stdin serial do QEMU após flush; isso não prova,
isoladamente, que o QEMU os entregou ao convidado. A leitura RBR e os valores
registrados são a evidência de recepção nesta fixture. OE é um indicador
booleano acumulado do LSR, não uma contagem de bytes perdidos. O ensaio não
estabelece taxa máxima, perda zero geral nem comportamento em UART física.

Os logs de build e de comando estão em [build inicial](build-uart-rz025-1.log),
[build final](build-uart-rz025-final.log), [comando anterior 1](comando-uart-rz025-1.log),
[comando anterior 2](comando-uart-rz025-2.log), [comando final 1](comando-uart-rz025-final-1.log)
e [comando final 2](comando-uart-rz025-final-2.log). A
[validação normal](uart-validacao/) contém cinco boots, 40 testes de pacotes
(3 + 28 + 9), 34 testes do núcleo, `cargo fmt --all --check` e manifests de
[fontes](uart-validacao/fontes.sha256) e [artefatos](uart-validacao/artefatos.sha256);
o manifest de artefatos inclui a imagem opt-in `saida/teste-uart-rz025/raiz-os.img`.
Os manifests identificam a fonte e a imagem compilada; os logs de comando
registram as duas invocações do teste. Os `qemu-*-stderr.log` copiados com os
relatórios dos boots normais são históricos de 09/11/2026 e não correspondem a
estas execuções. Os `qemu-stderr.log` próprios dos dois ensaios opt-in estão em
seus diretórios e estão vazios.

T03-16 está coberto neste recorte de duas fases finitas em QEMU; isso não fecha
RZ-025 nem o marco 0.3. O status e os limites permanecem na [matriz T03](../execucao-rz-023-20261115/MATRIZ.md).

## §14 — comparação de consultas vazias com a base 0.2

Esta comparação registra contagens nos pontos instrumentados da seção 14 da
especificação. A base é a tag `projeto-base-0.2.0`, commit
`1ce09444842c2f9e327479c9f7510fa91c99640e`; a [instrumentação aplicada](comparacao-base-020/instrumentacao.patch),
os [manifests de fontes](comparacao-base-020/fontes.sha256) e [artefatos](comparacao-base-020/artefatos.sha256)
estão preservados em `comparacao-base-020/`.

As três execuções da base foram aprovadas no perfil QEMU/TCG, pc/i440fx, 1 CPU,
256 MiB, OVMF e e1000: [1794745689237152200](comparacao-base-020/teste-comparacao-vazia-1794745689237152200/resultado.txt)
([serial](comparacao-base-020/teste-comparacao-vazia-1794745689237152200/serial.log)),
[1794746493934192700](comparacao-base-020/teste-comparacao-vazia-1794746493934192700/resultado.txt)
([serial](comparacao-base-020/teste-comparacao-vazia-1794746493934192700/serial.log))
e [1794746497858384700](comparacao-base-020/teste-comparacao-vazia-1794746497858384700/resultado.txt)
([serial](comparacao-base-020/teste-comparacao-vazia-1794746497858384700/serial.log)).
Em cada ensaio, os dois contadores foram, respectivamente, `29230`, `27887` e
`27297`: chamadas de `receber()` que retornaram `None` e voltas sem IPC consumido
ou byte serial/PS/2 lido.

As três execuções atuais também foram aprovadas no mesmo perfil QEMU: [1794746764914154900](../teste-comparacao-vazia-1794746764914154900/resultado.txt)
([serial](../teste-comparacao-vazia-1794746764914154900/serial.log)),
[1794746769383594500](../teste-comparacao-vazia-1794746769383594500/resultado.txt)
([serial](../teste-comparacao-vazia-1794746769383594500/serial.log))
e [1794746773837725400](../teste-comparacao-vazia-1794746773837725400/resultado.txt)
([serial](../teste-comparacao-vazia-1794746773837725400/serial.log)).
Cada resultado registra `99` para cada um dos dois contadores. Em ambas as
revisões, os marcadores delimitam exatamente 100 tiques. Os contadores são
atualizados em pontos do loop; a volta final pode atravessar a borda da janela,
portanto não são uma amostragem contínua alinhada às duas bordas.

Nos seriais das seis execuções há apenas a armação e o resultado impressos na
janela, sem estímulo externo adicional após o comando. Isso não prova ausência
de IPC interno nem de todos os outros eventos possíveis. `None` significa que
`receber()` não devolveu uma mensagem entregável, inclusive se entradas de
origens obsoletas tiverem sido purgadas; não prova que a FIFO física esteja
vazia nem representa todos os tipos de espera. A chamada finita atual a
`esperar_irq_ipc` pode terminar por timeout e esse timeout não incrementa o
contador de `receber()`. No loop atual, um IPC do multiplexador fica em
`ipc_pendente`: sua chegada e seu consumo podem marcar atividade em duas voltas;
esse estágio não existe no loop base 0.2.

As métricas descrevem consultas e voltas nos pontos instrumentados. Não são
equivalentes a custo de CPU, ciclos, utilização, energia, desempenho ou taxa de
economia; não se calcula percentual entre revisões. As três repetições da base
também mostram variação entre execuções, então diferenças isoladas não devem
ser tratadas como efeito estável. Os tempos do harness incluem o boot e não
representam a duração de 100 tiques em tempo de parede.

A rastreabilidade atual está em [comparacao-atual-validacao](comparacao-atual-validacao/),
com manifest de [fontes](comparacao-atual-validacao/fontes.sha256) e manifest de
[artefatos](comparacao-atual-validacao/artefatos.sha256), que inclui a imagem
opt-in, além de [build](build-atual-comparacao.log) e três [logs de comando](comando-atual-comparacao-1.log),
[2](comando-atual-comparacao-2.log), [3](comando-atual-comparacao-3.log), cinco boots
normais, 41 testes de pacotes, 34 do núcleo e `cargo fmt --all --check`. O
registro de [ambiente](comparacao-ambiente-posterior.log) foi coletado após os
seis boots de comparação e não prova que os executáveis e firmware permaneceram
imutáveis durante cada boot. Nos registros base, caminhos privados foram
redigidos com nota; os logs brutos permanecem no `target/` local da worktree.

Esta seção registra a comparação observada para §14; não fecha RZ-025, não
declara os T03 completos e não altera as limitações do marco 0.3.

## Empate de prazo e admissão IPC — T03-05

As execuções finais QEMU [1794749079973062400](../teste-prazo-empate-1794749079973062400/resultado.txt)
([serial](../teste-prazo-empate-1794749079973062400/serial.log)) e
[1794749084182557700](../teste-prazo-empate-1794749084182557700/resultado.txt)
([serial](../teste-prazo-empate-1794749084182557700/serial.log)) passaram; após
endurecer o parser, [1794749258177448900](../teste-prazo-empate-1794749258177448900/resultado.txt)
([serial](../teste-prazo-empate-1794749258177448900/serial.log)) também passou.
Em cada log, a espera registra prazo `136`, o timeout ocorre no tick `136`, e a
mensagem real da rede é admitida no mesmo tick. A chamada retorna `TEMPO` e a
chamada posterior recebe a IPC com origem, código e token esperados. É um
interleaving controlado em QEMU/TCG com CPU única; a evidência não distingue se
a mensagem permaneceu na FIFO ou se acordou uma segunda espera. Não generaliza
para todas as ordenações, nem para justiça sob fluxo ilimitado ou hardware.

As tentativas anteriores permanecem preservadas: [1794748241068193700](../teste-prazo-empate-1794748241068193700/resultado.txt)
falhou após o registro da sonda no ramo de syscall errado, com erro no segundo
retorno (`erro=0xfffffffffffffff6`); [1794748969907907600](../teste-prazo-empate-1794748969907907600/resultado.txt)
falhou porque o parser esperava geração 2 enquanto o serial registrava a geração
1. O oráculo final rejeita marcadores de falha mesmo se aparecerem depois do
marcador de resultado. Entre os manifests de fontes [anterior](prazo-empate-validacao/fontes.sha256)
e [final](prazo-empate-validacao/fontes-oraculo-final.sha256), somente
`ferramentas/src/main.rs` mudou. A [validação normal](prazo-empate-validacao/)
registra cinco boots, 42 testes de pacotes (3 + 30 + 9), 35 testes do núcleo,
`cargo fmt --all --check`, hashes de fonte/artefatos e logs de build/comando.
Os stderr copiados dos boots normais são históricos de 09/11/2026; não são
stderr destes ensaios opt-in.

## Modelo host de recepção — T03-02

O teste host [`recepcao_consume_mensagem_preexistente_e_acorda_uma_vez_apos_bloqueio`](../../nucleo/src/controle.rs#L602)
exercita mensagem já enfileirada antes da recepção, registro da espera quando a
caixa está vazia, bloqueio, admissão que acorda uma vez e segunda mensagem
mantida e consumida na ordem FIFO. O teste existente
[`recepcao_pendente_rearma_apos_revogacao_e_mantem_fifo_valida`](../../nucleo/src/controle.rs#L513)
exercita purga da origem revogada antes da seleção e novo bloqueio. Esses são
testes do modelo host compartilhado em `controle.rs`; não demonstram todas as
janelas do caminho integrado em VM. A validação de host e os cinco boots estão
nos logs de [prazo-empate-validacao](prazo-empate-validacao/).

Estas evidências parciais não concluem RZ-025, não alteram seu aceite e não
preenchem as lacunas restantes de T03.

## Recepção RX, quota por origem e controle supervisor — T03-12

As execuções QEMU [1794751363479086500](../teste-rx-quota-1794751363479086500/resultado.txt)
([serial](../teste-rx-quota-1794751363479086500/serial.log)) e
[1794751379029899500](../teste-rx-quota-1794751379029899500/resultado.txt)
([serial](../teste-rx-quota-1794751379029899500/serial.log)) foram aprovadas.
Em ambas, o host submeteu os IDs 1–18; o serviço de rede CPL3 reconheceu os
quadros e publicou IPCs associados aos IDs. Com 15 IPCs da rede já na fila, o
kernel recusou o ID 16 com `FILA_CHEIA` e `quota_rede=15`, evidenciando o limite
por origem. Depois, com 15 mensagens da rede e uma do supervisor, a ocupação
total chegou a 16. Após a drenagem conferida em ordem, o ID 16 foi reenviado e
aceito, seguido pelos IDs 17 e 18. O snapshot final registra 18 quadros,
máscara `0x3ffff`, `driver_invalido=0`, 18 IPCs de rede consumidas pelo terminal
e o controle do supervisor da referência atual.

As tentativas [1794750846647830200](../teste-rx-quota-1794750846647830200/resultado.txt)
e [1794750986962632100](../teste-rx-quota-1794750986962632100/resultado.txt)
permanecem preservadas: a primeira falhou no prefixo esperado pelo parser; na
segunda, o seletor consumiu IPC durante a barreira. Os dois resultados finais
e seus seriais são idênticos quanto aos marcos acima. O preflight documentado
no resultado incluiu ping, eco UDP comum e eco pelo hostfwd dedicado
`17779→7779`; o log do host registra submissões após `send`, enquanto a entrega
real é conferida separadamente no convidado. Trata-se de fluxo finito e em
ordem observada pelo driver; UDP não garante ordem. Não demonstra tráfego
ininterrupto, justiça sob carga ilimitada, perda zero geral, nem hardware físico.

A imagem opt-in preservada tem SHA-256
`0025c6d098de8c3cfdba33bdcf56caf1aac780a418a0ad02b713e11d5f115751`, conforme
[manifest dos artefatos preservados](rx-quota-validacao/artefatos-preservados.sha256).
A imagem e os EFI normal e opt-in foram preservados em
`target/preservacao-antes-rz022-20261115/artefatos-rx-quota-rz025/`.
A validação normal posterior foi concluída com exit 0: `cargo fmt --all --check`,
44 testes de pacotes (3 + 32 + 9), 35 testes do núcleo, compilação normal,
`verificar-imagem` e cinco boots em QEMU (smoke 1, isolamento 2, recuperação 2).
Os [logs e manifests](rx-quota-validacao/) incluem hashes do estado de artefatos
presente antes da compilação normal e do build normal, e as saídas novas de
relatórios e seriais estão em
[saidas-geradas-em-evidencias](rx-quota-validacao/saidas-geradas-em-evidencias/).
Os manifests foram calculados antes desta edição documental. Os 13 arquivos
históricos foram restaurados do backup; os cinco `qemu-*-stderr.log` históricos
não foram modificados nem copiados como novos. O log de
[restauração](rx-quota-validacao/restauracao-historicos.log) registra isso.

T03-12 é evidência parcial dentro de RZ-025. O item permanece em andamento; os
resultados não fecham o marco 0.3 nem preenchem os limites dos demais cenários.

## Limites revistos para T03-04 e T03-09

### T03-04 — evento recebido enquanto a rede geração 2 aguarda

O resultado [1794731396374947500](../teste-ciclo-espera-1794731396374947500/resultado.txt)
é `APROVADO`. O serial registra a rede geração 2 (`0x200000002`) bloqueada com
prazo `5129` e permanece ativa enquanto o harness avança além do prazo anterior
sem ultrapassar o prazo desta geração. Em seguida, o harness envia o comando de
liberação e exige a resposta terminal `REDE_LIBERADA geracao=2`, além de
`REDE_PRONTA` e conectividade posterior. No código da fixture, o terminal envia
a IPC `0x741` com geração 2 e payload `0xabc`, `0xdef`, `0x123`; a rede confere
origem, código, geração e payload e responde `0x742`, sem substituição da
instância durante esse intercâmbio; as gerações posteriores são reiniciadas
explicitamente pelo próprio ensaio ([envio](../../servicos/terminal/src/main.rs#L2282),
[validação e resposta](../../servicos/rede/src/main.rs#L754),
[condições do harness](../../ferramentas/src/main.rs#L2718)). O serial contém o
marcador `[teste-ciclo] REDE_LIBERADA geracao=2`; não imprime os códigos brutos
`0x741`/`0x742`. A validação desses campos decorre do caminho instrumentado e
das condições do harness aprovadas, não de uma linha serial que exponha os
códigos. Este ensaio não pressiona nem mede quota da fila.

### T03-09 — recusa antes da inicialização do driver

Na mesma fixture, a rede inicia com uma espera antes de chamar
`Rede::inicializar` ([ordem no serviço](../../servicos/rede/src/main.rs#L92)).
Enquanto a espera da geração 4 está pendente, o supervisor recusa a quinta
encarnação. Depois do timeout da espera, a rede inicializa. O serial
[1794731396374947500](../teste-ciclo-espera-1794731396374947500/serial.log)
registra `RECURSOS_ESGOTADOS`, depois `SERVICO_PRONTO` na geração 4 e ping
aprovado; o harness aprova também UDP posterior. Isso demonstra inicialização e
conectividade depois da recusa, não preservação de um dispositivo que já
estivesse ativo antes dela. A fixture não captura snapshots nem faz asserts das
filas. Esse recorte não amplia o status de RZ-025 nem fecha os cenários restantes
do marco 0.3.
