# ADR 009 — Limites de IPC e registros de espera

**Estado:** implementada e validada no recorte descrito.  
**Escopo:** ABI 2, serviços estáticos e uma CPU.

## Contexto

A ADR 003 prevê capacidade limitada e uma posição disponível para a outra
origem da caixa do terminal. A FIFO já possuía 16 posições, mas uma única
origem podia preenchê-las. Inscrição e espera já usam armazenamento fixo por
processo, sem alocação dinâmica.

## Decisão

Cada caixa mantém 16 posições; na caixa do terminal, cada ID lógico autorizado
ocupa no máximo 15. A contagem percorre as posições ocupadas, inclusive
referências de gerações distintas do mesmo ID. A purga usa referência completa;
retirada ou purga devolve imediatamente as posições efetivamente removidas.
Quota atingida e fila cheia retornam `FILA_CHEIA`, sem inserção nem despertar.
As demais caixas têm uma origem autorizada e conservam as 16 posições.

Cada processo tem uma espera e um slot de observação persistente. Uma segunda
espera retorna `ESTADO`; inscrição duplicada retorna `ESTADO`, e outra referência
enquanto o slot está ocupado retorna `RECURSOS`. Cancelamento da inscrição
exige a referência exata; sua invalidação terminal retém o slot até consumo ou
cancelamento. O término do observador remove seus registros. A seleção de
prazos e a substituição operam sobre o processo publicado sob exclusão na CPU
única. As regras detalhadas constam das ADRs 007 e 008 e da ABI.

## Alternativas

Contadores por origem evitariam uma varredura de até 16 entradas, mas exigiriam
atualizações consistentes em retirada, purga e substituição. Contar por geração
permitiria multiplicar o limite lógico. Reservar uma fila separada para cada
origem alteraria a ordem FIFO compartilhada.

## Consequências

A varredura é limitada e não aloca memória; a FIFO preserva ordem global. A
reserva não garante progresso se o terminal não consumir mensagens. Não se
afirma quota de IRQ, suporte SMP ou capacidade de produção. A alteração mantém
ABI 2.

O [ensaio de quota](../evidencias/teste-quota-1794725073649578100/resultado.txt) passou em QEMU para 15 mensagens da rede, recusa da 16ª, reserva ao supervisor, FIFO e readmissão após drenagem. A ordem inversa e liberação após uma retirada ou purga foram testadas no host, não nesse ensaio de VM. A construção normal, verificação de imagem e cinco boots de regressão passaram; veja o [resumo da execução](../evidencias/execucao-rz-019-20261115/RESUMO.md). IRQ e RZ-020 em diante permanecem pendentes.
