# RZ-023 — fechamento do aceite do item

RZ-023 está **concluído no escopo de seu aceite original**. A regressão final
após rebuild aprovou 12 comandos opt-in em QEMU; os logs de comando, saídas e
manifests estão relacionados abaixo. Os testes host e cinco boots normais
também passaram. Isso atende o item RZ-023 do backlog, sem declarar completos
todos os T03 ou o marco 0.3. A [matriz](MATRIZ.md) mantém os limites e cenários
que ainda exigem cobertura específica, como reassociação IRQ por geração,
fluxo ilimitado e hardware físico.

Em duas execuções da imagem `teste-irq-ipc`, o terminal aguardou a confirmação
da IRQ usada no comando de preparação, e o host forneceu entrada UART real. A
primeira IRQ do estímulo liberou oito mensagens IPC da rede; uma retenção de
20 tiques, exclusiva do ensaio, controlou a janela. O terminal recebeu as oito
em ordem enquanto havia trabalho serial pendente e depois produziu a resposta
completa do comando `eco`. O verificador exige marcadores completos, sequências
IRQ válidas, bytes após o início medido e resposta distinta do comando digitado.
Resultados e logs: [primeira execução](../teste-irq-ipc-1794732759143200100/resultado.txt),
[segunda execução](../teste-irq-ipc-1794732763992269200/resultado.txt),
[serial da segunda](../teste-irq-ipc-1794732763992269200/serial.log),
[stderr da primeira](../teste-irq-ipc-1794732759143200100/qemu-stderr.log) e
[stderr da segunda](../teste-irq-ipc-1794732763992269200/qemu-stderr.log).
A sequência IRQ pode já ter sido confirmada quando o IPC é processado. O ensaio
não demonstra ambas as fontes pendentes no seletor do kernel, justiça sob carga
ilimitada, SMP ou comportamento em hardware físico.

Implementação diretamente relacionada: [harness QEMU](../../ferramentas/src/main.rs#L2885),
[oráculo do log](../../ferramentas/src/main.rs#L2991),
[instrumentação terminal](../../servicos/terminal/src/main.rs#L282) e
[envio de IPC do serviço de rede](../../servicos/rede/src/main.rs#L177).

No host, três casos novos exercitam o slot IRQ e FIFO cheia, timeout sem
consumir IPC/IRQ e recusa de confirmação antiga após nova entrega. Passaram
`cargo fmt --all --check`, 29 testes dos pacotes e 34 testes de `nucleo --lib`:
[pacotes](testes-pacotes.log), [núcleo](testes-nucleo.log). A imagem normal foi
recompilada e verificada; `cargo raiz testar`, `testar-isolamento` e
`testar-recuperacao` passaram em cinco boots, com relatórios preservados nesta
pasta. Os testes host novos estão em
[IRQ/FIFO cheia](../../nucleo/src/irq.rs#L107),
[precedência de prazo](../../nucleo/src/irq.rs#L133) e
[revogação de sequência IRQ](../../nucleo/src/irq.rs#L168).

Ambiente QEMU/TCG `pc`/i440fx, uma CPU, 256 MiB, OVMF e e1000; versões e
hashes do firmware constam do [registro RZ-022](../execucao-rz-022-20261115/ambiente.log).
As [fontes](fontes.sha256) e os [artefatos](artefatos.sha256) identificam os
bytes desta etapa. Os arquivos `comando-irq-ipc-antes-auditoria-1.log` e
`comando-irq-ipc-antes-auditoria-2.log` são as duas execuções identificadas como
anteriores à revisão do oráculo; permanecem preservados e não substituem as duas
execuções finais vinculadas acima. A execução exploratória anterior, registrada
em `comando-irq-ipc-1.log`, também permanece preservada.

Os cinco arquivos `qemu-*-stderr.log` copiados nesta pasta são vazios e datados
de 09/11/2026. São cópias históricas, não atribuíveis às execuções de 15/11/2026.
Os seriais, relatórios e logs de comando atuais permanecem preservados; os
stderr das duas execuções atuais do ensaio opt-in estão nos links acima. O PS/2
físico, hardware físico e fallback com kernel anterior não foram validados aqui.

## Validação RX posterior — T03-22

Após os ensaios IRQ/IPC acima, duas execuções finais do ensaio opt-in de
orçamento RX passaram: [primeira](../teste-rx-orcamento-1794734545957669000/resultado.txt)
([serial](../teste-rx-orcamento-1794734545957669000/serial.log)) e
[segunda](../teste-rx-orcamento-1794734552405722300/resultado.txt)
([serial](../teste-rx-orcamento-1794734552405722300/serial.log)). Em ambas, o
preflight recebeu ping/eco UDP; antes da primeira consulta foram vistos 17 DD
ativos consecutivos, com `indice_rx=3`. A primeira consulta consumiu 16 quadros
válidos (`mascara=0xffff`, `esgotado=1`), `CEDER` retornou, e a consulta seguinte
processou o ID restante (`mascara_nova=0x10000`, total `0x1ffff`). Isso comprova
o esgotamento do orçamento nesta rajada e a continuação subsequente. O índice
registrado foi 3 nas duas execuções; elas não exercitam wrap do anel. A máscara
aceita qualquer permutação de 16 IDs distintos entre 0 e 16; os testes do
oráculo no host cobrem uma permutação alternativa em
[ferramentas/src/main.rs](../../ferramentas/src/main.rs#L5104), mas diferenças
de ordem UDP não foram observadas nos dois seriais QEMU. Os datagramas do ensaio
não geram respostas UDP.

O ensaio usa QEMU/TCG e hostfwd UDP controlado para os ports locais 17777/17778;
isso não representa uma rede externa, não mede carga contínua ou perda geral e
não demonstra comportamento em hardware físico. O marcador `CEDER_RETORNOU`
confirma apenas o retorno da syscall; não identifica qual processo executou
durante o intervalo. Veja também o [harness e oráculo](../../ferramentas/src/main.rs#L3085)
e a [instrumentação RX da rede](../../servicos/rede/src/main.rs#L293).

As tentativas anteriores permanecem preservadas: um preflight falhou por
timeout do eco UDP ([comando](comando-rx-orcamento-1.log),
[resultado](../teste-rx-orcamento-1794733787882074200/resultado.txt)). A tentativa
posterior que registrou `[FALHA fase=4]` usava o oráculo anterior; o comando
preserva também o timeout aguardando o marcador de continuação antigo
([resultado](../teste-rx-orcamento-1794734221237766300/resultado.txt),
[serial](../teste-rx-orcamento-1794734221237766300/serial.log),
[log de comando](comando-rx-orcamento-antes-permutacao-2.log)). Nenhuma tentativa
foi apagada ou apresentada como aprovada.

A validação posterior identificada separadamente em
[rx-validacao](rx-validacao/) registrou cinco boots normais: smoke, isolamento
e recuperação, com logs de comando, seriais e relatórios copiados nessa pasta.
Nela passaram 30 testes dos pacotes (18 + 3 + 9) e 34 testes do núcleo; o teste
IRQ/IPC mais recente está em [comando-irq-ipc.log](rx-validacao/comando-irq-ipc.log)
e aponta para seu resultado/serial. Formatação, imagem normal e os comandos de
smoke, isolamento e recuperação também têm logs nessa pasta. Os hashes da
fonte e dos artefatos dessa validação estão em
[fontes.sha256](rx-validacao/fontes.sha256) e
[artefatos.sha256](rx-validacao/artefatos.sha256). Os cinco arquivos
`qemu-*-stderr.log` copiados em `rx-validacao` têm data histórica de 09/11/2026
e não podem ser atribuídos às novas execuções; os seriais, relatórios e logs de
comando atuais permanecem preservados.

## Validação de classe IRQ — T03-13

Os resultados das duas execuções QEMU iniciais registraram APROVADO:
[primeira](../teste-irq-1794735243823061000/resultado.txt)
([serial](../teste-irq-1794735243823061000/serial.log)) e
[segunda](../teste-irq-1794735257443993000/resultado.txt)
([serial](../teste-irq-1794735257443993000/serial.log)). Nos dois seriais,
a IRQ4 real foi entregue com sequência 1. A rede tentou confirmar essa
sequência e recebeu `PERMISSAO`; então enviou uma mensagem IPC com código 3,
que não se fez passar pela confirmação da IRQ. O terminal confirmou depois a
sequência original legítima e o ciclo chegou à entrega/confirmação/rearme.
O ensaio cobre uma mensagem e a IRQ4 em QEMU; não demonstra resistência geral
a serviços comprometidos, outras classes de evento ou outros dispositivos.
O [harness e oráculo](../../ferramentas/src/main.rs#L562) verificam a distinção
entre resultado da syscall e código da mensagem.

Ressalva de auditoria histórica: estas duas execuções iniciais usaram um oráculo
que não rejeitava explicitamente `PERMISSAO` na syscall de diagnóstico `0xff00`;
seus marcadores de ausência de tempestade devem ser lidos com essa limitação.
Após a correção, o reteste [teste-irq-1794736006392076900](../teste-irq-1794736006392076900/resultado.txt)
passou; o [serial](../teste-irq-1794736006392076900/serial.log) registra
`MASCARA_SEM_TEMPESTADE`. Não há contagem de amostras ou duração impressa nesse
resultado para atribuir medidas adicionais.

A validação normal posterior, preservada em
[irq-classe-validacao](irq-classe-validacao/), inclui cinco boots: smoke,
isolamento e recuperação, com seus logs, seriais e relatórios. Nessa pasta
constam `cargo fmt --all --check`, 31 testes de pacotes (3 + 19 + 9), 34 testes
do núcleo, build e verificação da imagem normal. Os manifests de
[fontes](irq-classe-validacao/fontes.sha256) e
[artefatos](irq-classe-validacao/artefatos.sha256) identificam essa validação.
Os cinco `qemu-*-stderr.log` dessa pasta são arquivos históricos datados de
09/11/2026 e não podem ser atribuídos a estes boots; os seriais, relatórios e
logs de comando atuais estão preservados.

## Ensaio local de cancelamento — T03-24

O [ensaio opt-in em QEMU](../teste-cancelamento-aplicacao-1794736001073296900/resultado.txt)
passou; consulte também o [serial](../teste-cancelamento-aplicacao-1794736001073296900/serial.log).
A fixture correlacionou dois IDs aceitos e recebidos pelo serviço: cancelou
localmente `0x2401`, manteve `0x2402` ativo e liberou a resposta ativa primeiro;
a resposta tardia do ID cancelado foi correlacionada e ignorada, preservando o
estado local cancelado. O ensaio não demonstra cancelamento remoto, rollback,
idempotência ou a política geral do produto.

A validação normal posterior em [cancelamento-validacao](cancelamento-validacao/)
inclui cinco boots (smoke, isolamento e recuperação), `cargo fmt --all --check`,
32 testes de pacotes (3 + 20 + 9), 34 testes do núcleo, build e verificação da
imagem normal. Os manifests de [fontes](cancelamento-validacao/fontes.sha256)
e [artefatos](cancelamento-validacao/artefatos.sha256) identificam essa etapa.
Os cinco `qemu-*-stderr.log` copiados nessa pasta têm data histórica de
09/11/2026 e não são atribuíveis aos boots atuais; logs de comando, seriais e
relatórios atuais permanecem preservados.

## Regressão final do aceite RZ-023

Os 12 comandos abaixo foram executados depois de rebuild; os [manifests de
fontes](regressao-espera/fontes.sha256) e [artefatos](regressao-espera/artefatos.sha256)
identificam as entradas e imagens. Cada link de comando aponta ao log que
registra a execução e o diretório de evidências correspondente.

| Cenário | Comando e resultado |
| --- | --- |
| Recepção bloqueante | [log](regressao-espera/comando-receber-bloqueante.log) · [resultado](../teste-receber-bloqueante-1794740363976665500/resultado.txt) |
| Prazo | [log](regressao-espera/comando-prazo.log) · [resultado](../teste-prazo-1794740374912624600/resultado.txt) |
| Prazo em idle | [log](regressao-espera/comando-prazo-idle.log) · [resultado](../teste-prazo-idle-1794740384401729200/resultado.txt) |
| Ciclo de espera | [log](regressao-espera/comando-ciclo-espera.log) · [resultado](../teste-ciclo-espera-1794740394543076300/resultado.txt) |
| Observação de vida | [log](regressao-espera/comando-observacao.log) · [resultado](../teste-observacao-1794740429330454500/resultado.txt) |
| Notificação de vida | [log](regressao-espera/comando-vida.log) · [resultado](../teste-vida-1794740446416127900/resultado.txt) |
| Quota e saturação | [log](regressao-espera/comando-quota.log) · [resultado](../teste-quota-1794740468722158600/resultado.txt) |
| IRQ | [log](regressao-espera/comando-irq.log) · [resultado](../teste-irq-1794740489065000300/resultado.txt) |
| UART | [log](regressao-espera/comando-uart.log) · [resultado](../teste-uart-1794740498833412900/resultado.txt) |
| Seletor IRQ/IPC | [log](regressao-espera/comando-seletor-irq-ipc.log) · [resultado](../teste-seletor-irq-ipc-1794740507865847200/resultado.txt) |
| Orçamento RX | [log](regressao-espera/comando-rx-orcamento.log) · [resultado](../teste-rx-orcamento-1794740516890899300/resultado.txt) |
| Cancelamento local | [log](regressao-espera/comando-cancelamento-aplicacao.log) · [resultado](../teste-cancelamento-aplicacao-1794740528244617000/resultado.txt) |

A validação host e os cinco boots normais finais estão em
[irq-morte-validacao](irq-morte-validacao/): os logs de
[smoke](irq-morte-validacao/comando-smoke.log),
[isolamento](irq-morte-validacao/comando-isolamento.log) e
[recuperação](irq-morte-validacao/comando-recuperacao.log), com os relatórios
[smoke](irq-morte-validacao/RELATORIO-SMOKE.md),
[isolamento](irq-morte-validacao/RELATORIO-ISOLAMENTO.md) e
[recuperação](irq-morte-validacao/RELATORIO-RECUPERACAO.md), registram os boots.
Passaram 35 testes de pacotes e 34 do núcleo, além de
`cargo fmt --all --check`, build e verificação da imagem. Os manifests de
[fontes](irq-morte-validacao/fontes.sha256) e
[artefatos](irq-morte-validacao/artefatos.sha256) identificam essa validação.
Os stderr históricos de 09/11 nessa pasta permanecem identificados como não
atribuíveis a esses boots.
As tentativas e relatórios históricos malsucedidos descritos nas seções acima
permanecem em seus diretórios originais; esta regressão não os substitui.

## Seletor real IRQ/IPC — T03-21

Duas execuções QEMU do seletor passaram: [primeira](../teste-seletor-irq-ipc-1794738618371671800/resultado.txt)
([serial](../teste-seletor-irq-ipc-1794738618371671800/serial.log)) e
[segunda](../teste-seletor-irq-ipc-1794738643538232200/resultado.txt)
([serial](../teste-seletor-irq-ipc-1794738643538232200/serial.log)). O trace do
kernel registra IRQ4 pendente e duas IPCs elegíveis. Na primeira chamada,
`preferir_irq=0` escolheu IPC; na seguinte, `preferir_irq=1` escolheu IRQ. A
sequência de IRQ foi confirmada e a segunda IPC permaneceu na FIFO até ser
recebida. Isso demonstra a sobreposição finita observada, não fluxo ilimitado
nem justiça geral.

As tentativas anteriores permanecem preservadas: a primeira
[falhou por timeout da sonda](../teste-seletor-irq-ipc-1794738222011274300/resultado.txt)
([serial](../teste-seletor-irq-ipc-1794738222011274300/serial.log)); o serial
registra `FALHA_TIMEOUT_SONDA`, sem determinar a causa. A possibilidade de IPC
ter sido drenada não está confirmada. A segunda
[também falhou por timeout](../teste-seletor-irq-ipc-1794738417205755800/resultado.txt)
([serial](../teste-seletor-irq-ipc-1794738417205755800/serial.log)); o
diagnóstico registrou duas IPCs, mas IRQ ausente após a leitura antecipada do
byte. Não se atribui uma causa adicional. O ensaio IRQ/IPC antigo foi repetido
e aprovado em [teste-irq-ipc-1794738623175203500](../teste-irq-ipc-1794738623175203500/resultado.txt)
([serial](../teste-irq-ipc-1794738623175203500/serial.log)).

A validação normal posterior em [seletor-validacao](seletor-validacao/)
inclui cinco boots (smoke, isolamento e recuperação), `cargo fmt --all --check`,
33 testes de pacotes (3 + 21 + 9) e 34 testes de núcleo. O log de núcleo com
feature mostra 35; ele contém os mesmos 34 testes, portanto não se soma ao total
regular. Build, verificação da imagem e manifests de [fontes](seletor-validacao/fontes.sha256)
e [artefatos](seletor-validacao/artefatos.sha256) também estão nessa pasta. Os
cinco `qemu-*-stderr.log` copiados nela são históricos de 09/11/2026 e não podem
ser atribuídos aos boots atuais; seriais, relatórios e logs de comando atuais
permanecem preservados.
## Revogação de IRQ após falha do terminal — T03-19

O ensaio foi aprovado em duas execuções QEMU: [primeira](../teste-irq-morte-1794740207660657100/resultado.txt)
([serial](../teste-irq-morte-1794740207660657100/serial.log)) e
[segunda](../teste-irq-morte-1794740213828014200/resultado.txt)
([serial](../teste-irq-morte-1794740213828014200/serial.log)). Nos dois seriais,
o terminal recebeu IRQ4 real de sequência 31 e referência `0x100000001` antes
de falhar em UD2. O kernel registrou revogação da associação com máscara 1;
após o serviço receber `FALHOU`, o sinal opt-in do supervisor foi aceito. A rede
respondeu a eco UDP antes e depois da falha.

Cada execução registrou 50 amostras limitadas de IRQ0, com máscara correta e
associação ausente em 50/50 amostras, e zero entradas IRQ4 nessa janela. Isso
não prova ausência fora da janela nem execução contínua, não identifica qual
serviço executou entre amostras, e não testa reinicialização do terminal; é
evidência somente do cenário QEMU ensaiado.

As falhas anteriores estão preservadas: o [preflight](../teste-irq-morte-1794739904022393700/resultado.txt)
falhou aguardando eco UDP; o [ensaio com igualdade no harness](../teste-irq-morte-1794740078275520100/resultado.txt)
expirou aguardando o marcador `TIMER_AMOSTRAS quantidade=50`. Não são
substituídos pelos resultados aprovados.

A validação normal posterior em [irq-morte-validacao](irq-morte-validacao/)
inclui cinco boots (smoke, isolamento e recuperação), `cargo fmt --all --check`,
35 testes de pacotes (3 + 23 + 9), 34 testes do núcleo, build e verificação da
imagem normal. Os manifests de [fontes](irq-morte-validacao/fontes.sha256) e
[artefatos](irq-morte-validacao/artefatos.sha256) identificam a etapa. Os cinco
`qemu-*-stderr.log` copiados na pasta são históricos de 09/11/2026 e não podem
ser atribuídos aos boots atuais; seriais, relatórios e logs de comando atuais
permanecem preservados.
