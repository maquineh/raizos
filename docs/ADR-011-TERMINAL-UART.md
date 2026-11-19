# ADR 011 — Recepção UART por IRQ no terminal

**Estado:** implementada e validada no recorte de RZ-021.

## Contexto

O terminal precisa receber COM1 sem consultar RX a cada retorno do prazo do PS/2. A ADR 010 define a associação IRQ4 exclusiva e a confirmação por sequência na ABI 2. O controlador PIC por borda exige rechecagem do dispositivo no rearme.

## Decisão

O terminal arma IRQ4, habilita somente a interrupção de recepção da UART e usa espera multiplexada de IRQ/IPC com prazo de um tique para consultar PS/2. Após armação inicial, entrega IRQ ou rechecagem com dados, mantém trabalho serial local; processa até um byte por rodada e alterna a prioridade com PS/2. A sequência entregue permanece guardada até a drenagem; ao encontrar FIFO vazia, confirma e verifica novamente RX antes de dormir. Uma notificação sem byte também é confirmada. Timeout e IPC não iniciam polling de RX. LSR é lido também para transmissão; os indicadores de erro observados são acumulados e relatados.

Se a associação ou a espera falhar, o terminal registra o erro e volta a consultar serial por polling, com fonte UART desabilitada em falha após associação. A associação já criada pode permanecer no kernel; não há chamada de cancelamento neste contrato. Kernel anterior pode retornar `CHAMADA`. O teste IRQ anterior conserva sua própria configuração.

## Alternativas

Polling constante de RX continuaria consumindo tempo periódico e poderia mascarar uma interrupção perdida. Processar a FIFO inteira sem limite reduziria oportunidades para IPC e teclado. Uma IRQ por byte não é suposta: FIFO e timeout de recepção podem agrupar dados.

## Consequências e verificação

O [ensaio instrumentado](../evidencias/teste-uart-1794726895277854900/resultado.txt) passou com comandos e respostas completas, incluindo janela de rearme controlada. O [ensaio normal](../evidencias/teste-uart-1794726920528122700/resultado.txt) passou; construção, verificação de imagem e cinco boots normais também passaram. Os ensaios adicionais de IRQ, quota, vida e prazo-idle passaram, assim como 31 testes do núcleo e 23 dos pacotes no host. O [ensaio PS/2 emulado via HMP](../evidencias/execucao-rz-021-20261115/ps2-20261115-072013-2301210/resultado.txt) executou o comando sobre e observou resposta distinta. A janela usa instrumentação opt-in; as rajadas ensaiadas foram de quatro bytes a cada 50 ms. Esses resultados não demonstram perda zero sob carga arbitrária, tempo real, hardware físico, IRQ compartilhada ou SMP. PS/2 físico e fallback com kernel antigo não foram validados em VM.

Como validação posterior, o [resumo RZ-022](../evidencias/execucao-rz-022-20261115/RESUMO.md)
registra que os ensaios normal e instrumentado de UART passaram novamente, além
da imagem normal, regressões e ensaio de PS/2 emulado por HMP. Isso preserva o
histórico de RZ-021 sem sugerir que o terminal normal ainda consulta UART por
polling: o caminho normal aguarda IRQ4/IPC; polling serial permanece somente no
fallback após erro de associação/espera, e esse fallback com kernel antigo não
foi validado em VM. RZ-022 foi concluído no escopo de seu resumo; RZ-023 está em
andamento para inventariar a cobertura T03, conforme a
[matriz inicial](../evidencias/execucao-rz-023-20261115/MATRIZ.md). O marco 0.3
permanece incompleto.
