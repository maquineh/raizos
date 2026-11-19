# ADR 012 — Integração de espera nos serviços (RZ-022)

**Estado:** decisão de integração aplicada; RZ-022 concluído no escopo validado.  
**Escopo:** ABI 2, três serviços existentes, uma CPU e a plataforma QEMU de referência.

## Contexto

As extensões aditivas da ABI 2 permitem que os serviços aguardem IPC, vida,
interrupção atribuída ou prazo. O RZ-022 adapta terminal, rede e supervisor,
preservando suas responsabilidades em ring 3, a política manual, as gerações e
o orçamento de até quatro encarnações da rede. A [ADR 011](ADR-011-TERMINAL-UART.md)
já definiu o caminho do terminal com UART COM1/IRQ4; esta decisão registra os
comportamentos integrados dos três serviços e não altera aquela decisão.

## Decisão

- **Supervisor:** mantém inscrição de vida para a referência atual da rede e
  multiplexa eventos de vida e IPC. No modo automático, uma rede saudável espera
  sem prazo; uma falha ou término arma backoff de 100, 200 ou 400 tiques e espera
  apenas o prazo restante. Evento de vida, timeout e ativação do modo automático
  reconciliam pontualmente a referência e o estado. A nova instância só é
  observada depois de consumir a invalidação da anterior. Modo manual não
  reinicia pela notificação. Se o kernel retornar `CHAMADA` para as extensões,
  o supervisor preserva o fallback legado de consulta cooperativa e `CEDER`.
- **Rede:** espera IPC com prazo finito de 10 tiques para voltar a consultar a
  NIC e seus anéis. Processa até 16 quadros por rodada; se o orçamento de RX se
  esgotar, cede e continua elegível, sem dormir com trabalho possivelmente
  pendente. A e1000 continua em polling; não recebe IRQ neste escopo. Durante a
  inicialização, aguarda dois tiques antes da primeira releitura após reset e
  consulta a conclusão do reset por esperas curtas de um tique, até o limite de
  50 tiques. A fixture opt-in também exercita um prazo de 500 tiques.
- **FIFO durante inicialização da rede:** mensagens consumidas enquanto o driver
  espera ficam numa FIFO local limitada a 16 itens, além da caixa IPC de 16
  mensagens do kernel. A FIFO local preserva ordem e não cresce sem limite; ao
  saturar, a inicialização falha explicitamente, sem sobrescrever mensagens. A
  fixture aprovada observou 16 mensagens no armazenamento local, outras 16 na
  caixa do kernel, recusa da 33ª admissão com `FILA_CHEIA` e confirmação dos
  ACKs 1..32 em ordem após o prazo.
- **Compatibilidade e saturação:** `RECEBER_COM_PRAZO` retorna `CHAMADA` em
  kernel ABI 2 anterior à extensão. A rede abandona a espera por prazo e mantém
  polling cooperativo com `CEDER`. O supervisor também mantém seu fallback de
  consulta e `CEDER` sem notificações persistentes. Outros erros de espera são
  reportados e desativam aquela extensão; mensagens e armazenamento permanecem
  limitados. A integração não muda a versão ABI 2 nem promete suporte de
  fallback validado em VM para todo kernel anterior.

## Alternativas

Continuar consultando estado saudável no supervisor mantém atividade sem
trabalho; foi substituído por espera sem prazo enquanto a inscrição de vida está
ativa. Espera sem prazo na rede esconderia trabalho novo nos anéis da NIC; por
isso a rede usa um prazo curto de polling. Uma fila local sem limite ou que
descartasse mensagens durante o reset violaria a memória limitada e a FIFO; a
capacidade fixa e o erro explícito foram mantidos.

## Consequências e validação

O [ensaio integrado final](../evidencias/execucao-rz-022-1794731439514596800/resultado.txt)
passou em dois boots QEMU com imagem opt-in separada da normal: saturação
combinada da FIFO local e da caixa do kernel, ordem dos 32 ACKs, modo manual,
reset, prazo de RX de 10 tiques e retentativas automáticas com diferenças
observadas de 100, 200 e 400 tiques. O [ciclo de espera](../evidencias/execucao-rz-022-20261115/RESUMO.md)
passou duas vezes após reforço do harness, incluindo os marcadores completos do
kernel para as gerações 2, 3 e 4, cancelamento e preservação de prazos.

Também passaram a compilação e verificação da imagem normal, smoke, isolamento,
recuperação, os ensaios complementares registrados no resumo, formatação, 28
testes dos pacotes host e 31 testes do núcleo. O [resumo da execução](../evidencias/execucao-rz-022-20261115/RESUMO.md)
reúne comandos, relatórios, hashes e ambiente. Tentativas anteriores, inclusive
falhas, continuam preservadas em seus diretórios.

RZ-022 está concluído no escopo declarado; RZ-023 começou posteriormente como
inventário de cobertura T03 (consulte a [matriz inicial](../evidencias/execucao-rz-023-20261115/MATRIZ.md)).
Essa matriz não registra critérios aceitos como prontos. O marco 0.3 não está
fechado e a ABI permanece 2. A e1000 continua em polling. Hardware físico, PS/2
físico e fallback em kernel antigo não foram validados em VM; os ensaios não
demonstram latência física, perda zero, IOMMU ou ausência de toda atividade
periódica.
