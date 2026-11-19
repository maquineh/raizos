# ADR 008 — Inscrição e notificações de ciclo de vida

**Estado:** implementada e exercitada no recorte supervisor→rede.  
**Escopo:** um slot supervisor→rede, CPU única e uma thread por serviço.

## Contexto

A observação pontual da ADR 007 só existe durante uma chamada. O supervisor
precisa observar transições também enquanto atende outras responsabilidades,
sem que uma FIFO de mensagens cheia impeça o evento de ciclo de vida.

## Decisão

As chamadas opcionais ABI 2 `INSCREVER_VIDA`, `CANCELAR_VIDA` e `ESPERAR_EVENTO`
criam uma inscrição vinculada à referência exata, cancelam-na e multiplexam
IPC e ciclo de vida. Apenas supervisor pode observar rede. A inscrição valida
autoridade e identidade e lê estado inicial atomicamente sob IF=0, sem produzir
um evento redundante. Um único slot limitado por par lógico é independente da
caixa IPC; seus erros e representação estão na [ABI 0.2](ABI-0.2.md).

Mudanças coalescem o estado mais recente. Ao substituir a instância, a invalidação
terminal conserva o último estado observado antes da revogação: executável se
a instância estava ativa, falhou se havia falhado. A finalização interna usada
na troca não é publicada como transição adicional. O evento terminal não é
sobrescrito e reserva o slot até consumo ou cancelamento; a nova geração não o
herda nem pode ocupar o slot antes disso. Cancelamento aceita a referência
inscrita mesmo que já antiga, após verificar autoridade separadamente.

A espera verifica o prazo antes de consumir eventos, preserva ambas as fontes
no timeout e alterna IPC e vida quando ambas estão elegíveis. Consumir evento
completa a espera uma vez. A morte do observador cancela seu slot. Falha de
preparação ou quota esgotada não publica substituição. Eventos e inscrição
pertencem ao processo publicado; esta implementação não introduz callbacks nem
tokens assíncronos que possam alcançar outro processo por índice.

## Alternativas

Usar a FIFO comum permitiria que sua saturação ocultasse morte ou invalidação.
Uma prioridade fixa poderia privar uma classe; consultar continuamente o estado
não oferece registro atômico com a leitura inicial. Filas ilimitadas não se
ajustam ao orçamento estático do sistema.

## Consequências

O slot informa o estado relevante mais recente, não um histórico de transições.
Ainda não há observação genérica, IRQ, SMP, capabilities dinâmicas ou garantia
de progresso de um cliente que não executa. Kernel ABI 2 anterior retorna
`CHAMADA`; não se declara ABI 3 ou conclusão geral de 0.3. A validação deve
identificar testes e imagem efetivamente construídos. O [ensaio de vida](../evidencias/teste-vida-1794724542137348500/resultado.txt) e as regressões passaram em QEMU. Os cinco ensaios específicos anteriores reconstruídos passaram, conforme o [resumo da execução](../evidencias/execucao-rz-018-20261115/RESUMO.md); as regressões gerais também passaram novamente na imagem normal final.
