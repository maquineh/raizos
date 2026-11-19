# Resumo de execução — RZ-022

**Resultado: aprovado no escopo declarado.** Esta execução documenta o fechamento
do item RZ-022, a integração dos serviços sob ABI 2. Na data desta validação,
RZ-023 ainda constava como proposto e não havia sido executado nem concluído; o
inventário começou posteriormente em
[`MATRIZ.md`](../execucao-rz-023-20261115/MATRIZ.md). A cobertura completa dos
critérios T03 permanece sem aceite.

## Integração RZ-022

O teste integrado final passou em dois boots QEMU/TCG, com imagem opt-in separada
da imagem normal. No cenário manual, o reset da rede reteve 16 mensagens na FIFO
local do serviço e 16 na caixa IPC do kernel; a 33ª admissão recebeu
`FILA_CHEIA`. Após o prazo de 500 tiques, os ACKs 1–32 foram confirmados em
ordem. O modo manual não reiniciou a rede após UD2; o comando explícito criou a
geração 2, que respondeu a ping e UDP.

No teste de RX, a rede anunciou espera com prazo de 10 tiques. O harness observou
o marcador de timeout antes do eco UDP idêntico e do crescimento dos contadores
RX/UDP. Essa ordem não determina o instante físico em que o dispositivo recebeu
o datagrama. No cenário automático, as tentativas após as falhas foram separadas
por 100, 200 e 400 tiques; as gerações 2, 3 e 4 responderam a ping e UDP.

As referências diretas à instrumentação são o
[harness do teste RZ-022](../../ferramentas/src/main.rs#L3070), os
[marcadores do terminal](../../servicos/terminal/src/main.rs#L330),
[espera RX da rede](../../servicos/rede/src/main.rs#L315) e
[backoff do supervisor](../../servicos/supervisor/src/main.rs#L216).

O teste separado do ciclo de espera passou duas vezes com o harness corrigido.
Ele verifica as referências de geração antiga e nova pelo marcador completo do
kernel nas trocas para as gerações 2, 3 e 4, o cancelamento da espera antiga, a
preservação do prazo legado e o timeout da geração 4 após a recusa da quinta
encarnação. Os resultados e seriais estão nos diretórios de cada execução:
[resultado 1](../teste-ciclo-espera-1794731367103979200/resultado.txt),
[serial 1](../teste-ciclo-espera-1794731367103979200/serial.log),
[resultado 2](../teste-ciclo-espera-1794731396374947500/resultado.txt) e
[serial 2](../teste-ciclo-espera-1794731396374947500/serial.log). A implementação
do harness e dos seus oráculos está em
[executar_teste_ciclo_espera](../../ferramentas/src/main.rs#L1414) e
[janela_timeout_rede_ciclo](../../ferramentas/src/main.rs#L1342).

## Regressões e verificações registradas

- `cargo fmt --all --check` passou.
- Os testes host passaram: 28 testes nos pacotes `protocolos`, `ferramentas`,
  `contrato-boot` e `contrato-sistema`; 31 testes em `nucleo --lib`.
- A imagem normal foi recompilada e `cargo raiz verificar-imagem` confirmou GPT,
  FAT32 e `BOOTX64.EFI`.
- Os comandos normais de smoke, isolamento e recuperação passaram, com os
  relatórios copiados para esta pasta.
- Os ensaios adicionais de IRQ, quota, vida, prazo-idle, UART instrumentado e
  UART normal passaram. O PS/2 emulado por HMP executou o comando `sobre` e
  recebeu uma resposta distinta.

Logs de build, comandos, testes, verificação da imagem e os relatórios da imagem
normal estão preservados nesta pasta. Tentativas anteriores, inclusive as que
falharam, permanecem nos diretórios de evidência originais; este resumo aponta
para as execuções finais aprovadas sem sobrescrevê-las.

## Ambiente e rastreabilidade

Execução em 15 de novembro de 2026, QEMU/TCG `pc`/i440fx, uma CPU, 256 MiB,
OVMF e e1000. As versões de Rust/Cargo/QEMU e hashes dos firmwares estão em
[`ambiente.log`](ambiente.log). Os comandos efetivamente registrados estão nos
arquivos `comando-*-validado.log`; os resultados QEMU do ensaio integrado estão
em [resultado.txt](../execucao-rz-022-1794731439514596800/resultado.txt), e as
saídas seriais e stderr do teste integrado opt-in estão em
[manual](../execucao-rz-022-1794731439514596800/serial-manual.log) /
[stderr manual](../execucao-rz-022-1794731439514596800/qemu-manual-stderr.log) e
[automático](../execucao-rz-022-1794731439514596800/serial-automatico.log) /
[stderr automático](../execucao-rz-022-1794731439514596800/qemu-automatico-stderr.log).
Os logs dos comandos normais e relatórios preservados nesta pasta correspondem
às execuções atuais.

Os manifests [`fontes-validado.sha256`](fontes-validado.sha256) e
[`artefatos-validado.sha256`](artefatos-validado.sha256) registram hashes das
fontes e dos artefatos correspondentes à validação final.

Os cinco arquivos `qemu-*-stderr.log` copiados nesta pasta são arquivos vazios
datados de 09/11/2026. São cópias históricas e não podem ser atribuídos às
execuções de 15/11/2026. Os seriais, relatórios e logs de comando atuais
permanecem preservados; os stderr atuais do teste integrado opt-in estão nos
links manual e automático acima.

## Limites

A e1000 ainda usa polling de RX com prazo. O ensaio não mede o instante físico
de recepção nem latência em hardware. Não foram validados hardware físico, PS/2
físico ou fallback com kernel ABI 2 antigo; não há garantia geral de perda zero,
IOMMU, isolamento contra DMA ou ausência de toda atividade periódica. O terminal
e o supervisor continuam sem política de reinício. RZ-023 não foi executado por
esta validação; a matriz inicial relaciona evidências anteriores sem lhes
atribuir cobertura completa. O marco 0.3 segue aberto. A versão do produto
permanece 0.2.0 e a ABI permanece 2.
