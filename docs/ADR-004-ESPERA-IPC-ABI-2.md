# ADR 004 — Espera de disponibilidade IPC na ABI 2

**Estado:** adotada; implementação e integração exercitadas no recorte documentado.  
**Escopo:** caixa própria, uma CPU e uma thread por serviço.

## Contexto

O supervisor em modo manual só precisa atender IPC, mas antes disputava CPU
consultando a caixa vazia. A espera multiplexada da ADR 003 depende de prazo,
IRQ, inscrições e identidade de operação que ainda não estão implementados.

## Decisão

Adicionar à ABI 2 a chamada opcional `ESPERAR_IPC` número 11. A operação só
observa a caixa própria, não recebe referência nem concede autoridade sobre outra
caixa. Com fila não vazia retorna zero; caso contrário bloqueia até um envio
válido ser inserido. Consulta, registro e inserção são serializados com IF=0 na
única CPU. O envio continua não bloqueante e conserva seus erros e FIFO.

O retorno zero pede nova consulta via `RECEBER`, sem reservar ou consumir uma
mensagem. Purga por revogação pode deixar a caixa vazia antes do recebimento;
o chamador deve tratar isso e pode esperar novamente. Estado inválido retorna
`ESTADO`. Kernels ABI 2 antigos retornam `CHAMADA`; serviço novo deve manter
fallback de consulta e cessão. Chamadas existentes e seus layouts permanecem
inalterados, e a versão não muda para ABI 3. O supervisor desabilita tentativas
após erro inesperado e o notifica uma vez, mantendo cessão; não confunde esse
erro com o fallback esperado por `CHAMADA`. A compatibilidade com kernel antigo
decorre da análise do retorno `CHAMADA` e ainda não foi ensaiada em VM com o
serviço novo combinado ao kernel antigo.

O registro pertence ao processo e é cancelado ao terminar ou substituir sua
instância, depois de preparada a substituta. Ele não identifica callbacks,
timeouts ou inscrições assíncronas por geração e sequência; não pode ser usado
para implementar esses mecanismos sem ampliar sua identidade e validação.

## Alternativas

Manter apenas polling preservaria compatibilidade sem reduzir consultas vazias.
Alterar `RECEBER` para bloquear quebraria serviços existentes. Expor agora a
espera multiplexada exigiria antecipar os contratos e testes ainda pendentes.

## Consequências

O modo manual pode dormir aguardando IPC, mas serviços com trabalho periódico
ou outras fontes devem continuar sua política de polling até existir espera
adequada. Uma única CPU e IF=0 são pressupostos, não sincronização SMP. A
extensão não oferece prazo, IRQ, notificação de morte nem cancelamento remoto,
e não conclui RZ-015 ou o marco 0.3. Testes de host, imagem e regressão devem
ser associados à construção efetivamente executada. Os resultados e limites
da execução desta extensão estão no [resumo](../evidencias/execucao-supervisor-20261115/RESUMO.md).
