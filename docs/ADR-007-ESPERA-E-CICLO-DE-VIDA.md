# ADR 007 — Espera e ciclo de vida

**Estado:** implementada e exercitada no recorte de observação pontual autorizada.  
**Escopo:** supervisor observa uma instância exata da rede, CPU única.

## Contexto

Uma espera na caixa própria não identifica qual serviço produziu a resposta
aguardada. A morte da rede poderia deixar o supervisor bloqueado sem informação.
Os prazos atuais são varridos no próprio slot publicado, sem callbacks externos.

## Decisão

A syscall opcional ABI 2 número 14 `RECEBER_OBSERVANDO` combina uma recepção
da própria caixa com observação one-shot de uma referência exata da rede e os
modos de prazo da ADR 006. Apenas o supervisor pode observar a rede; a referência
identifica a instância e não concede autoridade. O contrato está na [ABI 0.2](ABI-0.2.md).

Sob IF=0, a admissão verifica autoridade, referência, argumentos, vida e fila.
Alvo já falho ou finalizado retorna `ALVO_INDISPONIVEL`, referência antiga é
recusada, e alvo ativo sem mensagem na tentativa imediata retorna `AUSENTE`.
Durante a espera, falha, saída, encerramento ou substituição da instância exata
concluem com `ALVO_INDISPONIVEL`. Prazo já vencido prevalece; mensagem ainda
não consumida permanece na fila. Se a mensagem já foi entregue, a conclusão não
é alterada. Um resultado é único e a observação termina junto com a chamada.
Uma mensagem dessa geração pode permanecer na fila após falha, pois a falha não
troca a referência; o reinício ou encerramento autorizado realiza a purga. O
cliente precisa considerar a identidade de origem ao processá-la.

Ao morrer ou ser substituído, o próprio esperador perde seus registros de espera
e prazo sem retorno ao contexto antigo. O kernel prepara a nova instância antes
de revogar a antiga; falha de preparação ou esgotamento de quota conserva seus
registros e recursos. A expiração examina os slots publicados, não um callback
externo atrasado. Novas inscrições assíncronas precisarão de identidade completa
e sequência, não bastando confiar no índice. Não há alteração da quarentena DMA.

## Alternativas

Esperar apenas IPC poderia dormir até o prazo mesmo se a rede já tivesse falhado.
Uma inscrição persistente de ciclo de vida exige os slots, coalescência e
cancelamento de RZ-018 e não é criada por esta chamada. Enviar mensagem comum
para sinalizar morte misturaria evento privilegiado com IPC de usuário.

## Consequências

Não há observação genérica nem notificação persistente; se o serviço não estiver
esperando, deve consultar estado ou usar mecanismos futuros. O kernel impõe
autoridade, mas o supervisor continua decidindo recuperação. O modo manual
não reinicia a rede por receber esse erro. Kernels ABI 2 antigos retornam
`CHAMADA`, sem tradução ABI 1. Testes e regressões devem ser associados à imagem
reconstruída; os resultados e limites estão no [resumo da execução RZ-017](../evidencias/execucao-rz-017-20261115/RESUMO.md). Observação perante substituição e fallback de kernel anterior não foram ensaiados em VM. A ABI continua 2 e o marco 0.3 permanece incompleto.
