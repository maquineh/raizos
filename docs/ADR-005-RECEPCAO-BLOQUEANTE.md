# ADR 005 — Recepção bloqueante na ABI 2

**Estado:** implementada e exercitada no recorte de recepção da própria caixa.  
**Escopo:** IPC da própria caixa, CPU única e uma thread por serviço.

## Contexto

`RECEBER` consulta sem bloquear; `ESPERAR_IPC` só informa disponibilidade e a
mensagem pode ser purgada antes da consulta. Serviços que só aguardam IPC precisam
de uma recepção que volte com uma mensagem válida, sem perda de despertar.

## Decisão

Adicionar a chamada opcional ABI 2 número 12, `RECEBER_BLOQUEANTE`, com status
em `rax` e seis valores da mensagem em registradores separados. O contrato
preciso está em [ABI 0.2](ABI-0.2.md). O kernel só acessa a caixa do chamador;
autorização e referência exata do envio continuam verificadas no caminho de
`ENVIAR`. Fila cheia e demais recusas não causam despertar.

A verificação, a publicação da espera, a inserção e a seleção são serializadas
com IF=0 na única CPU. Envio admitido desperta sem entregar diretamente. Na
seleção, o kernel ignora origens revogadas, consome uma mensagem FIFO e preenche
o contexto; se uma purga esvaziou a caixa, rearma a espera e escolhe outro
processo. Uma mensagem passa a consumida na seleção, ainda que uma interrupção
ocorra antes da primeira instrução em usuário. Morte ou substituição descarta a
espera da instância; falha de preparação mantém a instância antiga e sua espera.

`RECEBER` e `ESPERAR_IPC` mantêm suas semânticas. Kernels ABI 2 anteriores
retornam `CHAMADA` para a nova operação, permitindo fallback; a versão não é
alterada para ABI 3. Códigos de mensagem e erros não se confundem, pois o status
é separado. Não se garante compatibilidade em VM até ensaiar a combinação.

## Alternativas

Entregar diretamente no envio deixaria um resultado já copiado ao contexto sujeito
a revogação antes da seleção. Alterar `RECEBER` quebraria serviços existentes.
Repetir espera de disponibilidade no serviço mantém uma janela de purga e não
oferece o contrato de uma única operação bloqueante.

## Consequências

O registro pertence à instância e não resolve referências e sequências para
eventos temporizados ou inscrições assíncronas futuras. Não há timeout, IRQ,
espera multiplexada ou cancelamento remoto. Terminal e rede precisam de suas
outras fontes e não podem dormir indefinidamente nessa chamada. A desativação
local de interrupções não é sincronização SMP. Ensaios pertinentes e regressões
devem ser associados ao binário reconstruído, sem aproveitar aprovação histórica. Os resultados e limites estão no [resumo da execução RZ-015](../evidencias/execucao-rz-015-20261115/RESUMO.md).
