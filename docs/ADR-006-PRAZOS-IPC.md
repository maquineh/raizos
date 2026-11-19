# ADR 006 — Prazos de recepção IPC na ABI 2

**Estado:** implementada e exercitada para recepção IPC da própria caixa.  
**Escopo:** caixa própria, CPU única, timer observado pelo núcleo e uma thread por serviço.

## Contexto

A recepção bloqueante da ADR 005 pode aguardar indefinidamente. Serviços com
trabalho periódico precisam de uma espera limitada, mantendo mensagens na FIFO
quando o prazo vence e sem confundir ausência imediata com timeout.

## Decisão

Adicionar a chamada opcional ABI 2 `RECEBER_COM_PRAZO` número 13, com modos e
duração explícitos definidos na [ABI 0.2](ABI-0.2.md). O prazo finito é a soma
modular do tique de admissão e da duração. `vencido` significa
`agora.wrapping_sub(prazo) < 2^63`; a duração é limitada a 2^63−1 e a expiração
deve ser processada antes de decorridos 2^63 tiques observados após o prazo.
O contador mede interrupções do timer observadas, não tempo de parede nem tempo
real enquanto interrupções estiverem mascaradas.

Sob IF=0, verifica-se prazo antes da entrega; uma mensagem ainda na FIFO não é
consumida se o prazo já venceu, mesmo que tenha chegado antes da seleção. Uma
entrega concluída antes do vencimento permanece concluída. IRQ0 em CPL 0 ou CPL 3
examina os registros das instâncias publicadas, inclusive as prontas após IPC;
envio e seleção também conferem o prazo. A expiração cancela a espera e a conclui
uma vez. Uma purga antes da entrega rearma a mesma espera com o prazo original.

O registro reside no processo e é removido em término ou substituição após a
preparação da nova instância; recusas de preparação preservam a anterior. A
varredura de slots atuais não usa callback atrasado nem token externo, e não
substitui identidade e sequência exigidas para futuras inscrições ou prazos
assíncronos. Não há cancelamento remoto nem notificações de IRQ nessa extensão.

Kernels ABI 2 anteriores retornam `CHAMADA` à syscall desconhecida. A mesma
resposta também indica argumentos inválidos; serviços devem usar valores válidos
e prover fallback adequado. Não há mudança para ABI 3.

## Alternativas

Polling por `TIQUES` mantém serviços executáveis sem trabalho. Um sentinela de
duração para representar ausência de prazo confundiria modo e valor. Fazer o
envio consumir a mensagem entregaria conteúdo que ainda poderia ser revogado.

## Consequências

O custo de varredura é limitado às três instâncias estáticas atuais. Desabilitar
interrupções localmente não serializa SMP. A implementação não promete precisão
física, não elimina polling de dispositivos e não conclui espera multiplexada
ou o marco 0.3. O [resumo da execução](../evidencias/execucao-rz-016-20261115/RESUMO.md) identifica a construção e testes efetivos. O supervisor normal ainda não usa a chamada com prazo; a combinação de serviço novo com kernel antigo não foi ensaiada em VM.
