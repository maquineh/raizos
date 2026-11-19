# ADR 003 — Espera, prazo e observação na etapa 0.3

**Estado:** decisão adotada para o desenho de 0.3; estados, idle e despertar por IPC em andamento, espera 0.3 completa ainda não implementada.  
**Escopo:** uma CPU, três serviços com uma thread cada e permissões estáticas.

## Contexto

A ABI 2 conserva consultas não bloqueantes e agora admite uma extensão opcional
de espera de disponibilidade IPC, descrita na [ADR 004](ADR-004-ESPERA-IPC-ABI-2.md).
Uma espera multiplexada precisa preservar FIFO, autoridade,
gerações e recuperação sem perder despertares ou tratar um serviço bloqueado
como falho. O desenho e os cenários futuros estão na
[especificação 0.3](projeto/ESPECIFICACAO-0.3.md).

## Decisão

- Cada instância ativa pode registrar uma espera, identificada pela referência
  completa e sequência não reutilizada. Verificação de eventos e publicação do
  bloqueio são atômicas na CPU única. Uma única conclusão remove os demais
  vínculos; eventos não consumidos permanecem pendentes. Esgotamento de sequência
  recusa nova espera, sem rollover silencioso.
- A espera multiplexa caixa própria, IRQ atribuída, observação autorizada e
  prazo. A classe do resultado é produzida pelo kernel, separada de mensagens de
  usuário. Envio continua não bloqueante; fila cheia é erro explícito. A consulta
  de vida distingue ativo, falhou e finalizado; bloqueado é estado de agendamento.
- Tentativa imediata consulta eventos e retorna ausência sem bloquear. Prazo
  finito usa duração de 1 a 2^63−1 tiques de contador monotônico de 64 bits;
  zero indica tentativa imediata e ausência de prazo é opção distinta. O prazo é
  calculado modularmente. `vencido = agora.wrapping_sub(prazo) < 2^63`;
  expirações devem ser processadas antes de decorridos 2^63 tiques observados
  após seu vencimento. Tiques observados não medem tempo físico com IRQ mascarada.
  Sob exclusão, prazo já vencido prevalece
  sobre evento ainda não entregue, sem consumi-lo; conclusão anterior persiste.
- Uma inscrição autorizada por observador e referência exata combina estado
  inicial e registro atômico. O estado inicial falho/finalizado não duplica a
  mesma transição; mudanças posteriores ocupam slot próprio, coalescendo estado
  mais recente. A invalidação é terminal, conserva a referência antiga até
  consumo e então libera o slot. Referência antiga é recusada sem inscrição;
  cancelamento ou morte do observador libera o slot. Não há redirecionamento.
- A caixa tem 16 mensagens FIFO. Na caixa do terminal, rede e supervisor têm
  limite de 15 mensagens pendentes cada, deixando uma posição para a outra
  origem; a recusa é fila cheia. Notificações usam armazenamento limitado
  separado: um slot por par observador/alvo lógico autorizado e por IRQ atribuída.
  Apenas supervisor pode observar rede no recorte inicial. Enquanto a invalidação
  da geração anterior ocupar o slot, inscrição da nova retorna recursos esgotados;
  após consumo ou cancelamento ela pode ser inscrita. Uma espera
  por instância e as associações estáticas limitam os registros; ausência de
  slot retorna recursos esgotados. Classes IPC, IRQ e ciclo de vida são
  selecionadas em rotação quando elegíveis.
- A morte do próprio esperador desvincula sua espera sem retorno ao processo
  antigo. Invalidação de alvo observado produz evento para observador vivo.
  Quota ou falha de preparação do sucessor não revoga a instância publicada nem
  altera seus registros, filas ou associações; após preparação bem-sucedida,
  revogação e publicação seguem a [ADR 002](ADR-002-CICLO-DE-VIDA.md).
- A IRQ piloto é COM1/IRQ4 exclusiva. Associação, máscara e reconhecimento são
  mediados pelo kernel; somente o proprietário autorizado rearma com sequência
  atual. A especificação estabelece a sequência de atendimento a ensaiar.

Referências e sequências identificam objetos, não concedem autoridade. Destino
inexistente, referência antiga, permissão negada, estado inválido, fila cheia,
recursos esgotados e argumentos/prazos inválidos têm resultados distintos;
timeout e ausência imediata também são distinguíveis. A ordem de validação não
autoriza alterar estado em erro. A codificação exata desses resultados, layout,
registradores, números de chamadas e número da futura ABI serão definidos e
testados antes da exposição, com reconstrução coordenada de kernel, contrato,
wrappers e serviços. Não há tradução implícita de binários ABI 2; a ABI 2 e a
imagem 0.2 continuam vigentes até uma entrega implementada e versionada. A
extensão aditiva `ESPERAR_IPC` da ADR 004 é mais restrita e não codifica a
espera multiplexada aqui decidida.

## Alternativas

Envio bloqueante acrescentaria espera por espaço e ciclos de dependência; fica
fora de 0.3. Usar a fila comum para eventos de kernel permitiria que sua
saturação ocultasse invalidação ou IRQ. Prioridade fixa entre classes poderia
privar outra classe; rotação oferece oportunidade condicionada ao consumo.
Contador de parede introduziria ajustes de relógio na semântica de prazo.

## Consequências e verificação

O kernel precisará de idle próprio, inclusive tratamento correto de frames de
IRQ em CPL 0 e CPL 3, além de transições, quotas e testes de corrida. Desligar
interrupções localmente não fornece sincronização SMP. A espera não cancela
pedido já aceito nem garante tempo real, confinamento DMA ou perda zero de
dispositivo. O supervisor conserva a política de recuperação e os limites de
arenas. Nenhum teste de 0.3 é alegado por esta decisão; a matriz T03 e os gates
da especificação continuam pendentes.
