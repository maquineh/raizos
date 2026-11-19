# Continuar o projeto em outra sessão

Este arquivo acompanha a pasta completa `raiz-os`. Ele não substitui as fontes,
a especificação ou as evidências do projeto.

## Contexto que precisa sobreviver

- Objetivo do usuário: SO próprio em Rust, microkernel próprio, sem base em outro
  SO, entregue como um produto integrado com boot, terminal, rede e evolução para
  aplicações e persistência.
- A base executável é 0.2.0, ABI 2. A organização do projeto acrescenta documentos;
  não implementa 0.3.
- Há carregador UEFI, núcleo x86-64, três serviços em ring 3, IPC com filas
  limitadas, preempção, rede básica e recuperação limitada da rede.
- Referências identificam instâncias por geração; a autorização continua
  estática. Não há capabilities dinâmicas nem IOMMU.
- A base permite a instância inicial da rede e três reinícios por boot. Arenas
  antigas não são reutilizadas. Terminal e supervisor não são reiniciáveis.
- Os 26 testes no host e cinco boots constam da validação histórica da 0.2.
  Conferir os relatórios não equivale a repetir os testes.
- O uso de Tanenbaum está documentado como aplicação dos conceitos de abstração
  e administração de recursos; não há selo ou checklist universal do autor.

## Tarefa inicial da próxima evolução

Leia a [ADR 003](../ADR-003-ESPERA-0.3.md), a [especificação](ESPECIFICACAO-0.3.md) e RZ-014 no [backlog](BACKLOG.csv). RZ-014 está concluído no escopo de estados, despertar e idle; uma imagem opt-in exercitou IPC real entre terminal e supervisor. A ABI 2 tem extensões de disponibilidade, recepção, prazo e inscrição persistente de vida. RZ-016 e RZ-017 estão concluídos nos recortes documentados; RZ-018 está concluído no recorte supervisor→rede e a [ADR 008](../ADR-008-NOTIFICACOES-VIDA.md) descreve IPC/vida supervisor→rede. Historicamente, no RZ-018 o supervisor automático ainda consultava estado e prazos; essa consulta contínua foi substituída no RZ-022 por espera de vida/IPC e prazo de retentativa. O ensaio de vida e as regressões em QEMU passaram. Os cinco ensaios específicos anteriores reconstruídos passaram, conforme o [resumo da execução](../../evidencias/execucao-rz-018-20261115/RESUMO.md); as regressões gerais também passaram novamente na imagem normal final. As regressões históricas da execução RZ-017 passaram; a combinação de kernel antigo e serviço novo não foi ensaiada em VM. Em modo de 64 bits, o frame de IRQ inclui SS e RSP também em CPL 0; o tratamento distingue a origem e preserva a pilha ociosa. Números e layouts da ABI futura precisam de definição e testes antes da exposição; a ABI vigente permanece 2.

O piloto de IRQ é COM1/IRQ4 exclusiva no QEMU de referência. Interrupções da e1000
e PS/2 não fazem parte desse piloto. A integração deve permitir polling com
espera por prazo onde ainda não existir entrega de IRQ.

Implemente incrementos que possam ser revisados e validados. Uma entrega completa
do marco exige o fluxo sem perda de despertar, deadlines determinísticos,
cancelamento de espera, filas limitadas, idle seguro e integração dos serviços,
além das regressões aplicáveis da base 0.2. Os ensaios parciais ainda não cobrem
todos esses cenários.

## Texto para entregar a um agente de programação

```text
Continue o projeto Raiz OS que acompanha esta mensagem.

Leia AGENTS.md, PROJETO.md, README.md e a documentação em docs/projeto/.
A versão implementada é 0.2.0, ABI 2. Não a substitua por um projeto novo.
O objetivo é um SO original em Rust, com microkernel e produto integrado.

RZ-019 está concluído no recorte implementado: a [ADR 009](../ADR-009-LIMITES-IPC.md) registra caixas de 16 mensagens, limite de 15 por origem lógica na caixa do terminal e slots estruturais de inscrição e espera. Os 29 testes do núcleo passaram no host; as regressões normais em QEMU passaram. RZ-020 está concluído no recorte de IRQ4 exclusiva. O [ensaio de quota em QEMU](../../evidencias/teste-quota-1794725073649578100/resultado.txt) passou para 15 envios da rede, recusa do 16º, reserva ao supervisor, FIFO e readmissão após drenagem. A ordem inversa e liberação após uma retirada ou purga foram testadas no host, não nesse ensaio de VM. O [resumo da execução](../../evidencias/execucao-rz-019-20261115/RESUMO.md) reúne construção normal, verificação de imagem e os cinco boots de regressão.

RZ-020 está concluído no recorte de IRQ4 exclusiva: a [ADR 010](../ADR-010-NOTIFICACAO-IRQ4.md) acrescenta à ABI 2 uma associação exclusiva de IRQ4 ao terminal, notificação coalescida separada da fila IPC e confirmação por sequência. No [ensaio opt-in em QEMU](../../evidencias/teste-irq-1794725963506321900/resultado.txt), IRQ4 acordou o terminal a partir de idle CPL 0; foram observadas negação a outro serviço, recusas de sequência incorreta e duplicada, rechecagem e rearme. Com B pendente e sem confirmação por 50 tiques, a máscara foi observada em duas amostras, o contador de entradas permaneceu igual e o timer avançou; C chegou após o rearme. Coalescência foi testada no host, não demonstrada nesse ensaio QEMU. Isso não demonstra ausência absoluta de tempestades nem suporte a hardware físico. A construção normal, verificação de imagem e três comandos de regressão em QEMU (cinco boots) passaram. Naquela execução o terminal normal ainda usava polling. SMP e IRQ compartilhada não foram demonstrados. Os ensaios reconstruídos de quota, vida e prazo-idle também passaram; o [resumo da execução RZ-020](../../evidencias/execucao-rz-020-20261115/RESUMO.md) reúne logs, hashes e limites.

RZ-022 está concluído no escopo validado. Consulte a [ADR 012](../ADR-012-ESPERA-SERVICOS-RZ022.md)
e o [resumo final](../../evidencias/execucao-rz-022-20261115/RESUMO.md), que
registra dois boots integrados, o ciclo de espera aprovado duas vezes, regressões
normais e ensaios complementares. RZ-023 também está concluído no escopo do
aceite original: 12 comandos opt-in após rebuild, testes host e cinco boots
normais estão no [resumo](../../evidencias/execucao-rz-023-20261115/RESUMO.md).
A [matriz T03](../../evidencias/execucao-rz-023-20261115/MATRIZ.md) mantém
limites e cobertura pendente por cenário; isso não conclui todos os T03 nem o
marco 0.3. RZ-024 foi concluído no recorte controlado de idle/despertar, com
resultados e limites no [resumo](../../evidencias/execucao-rz-024-20261115/RESUMO.md).
Preserve ABI 2, a fronteira de autoridade, as gerações e o limite de quatro
encarnações por boot.

O piloto de IRQ é UART COM1/IRQ4 exclusiva. Não suponha IRQ da e1000 resolvida
nem declare ABI 3 antes de definir e implementar o contrato correspondente.
Preserve alterações de outras pessoas. Não publique para um remoto sem
autorização. Informe os testes realmente executados e os limites restantes.

Use as evidências existentes como base histórica. Recompile antes de testar
código novo no QEMU. Não transforme um teste de lógica no host em uma
alegação de isolamento por hardware ou proteção contra DMA.
```

O texto acima autoriza a próxima evolução quando usado como tarefa. Neste pacote,
ele é apenas um roteiro de continuidade; não registra execução da versão 0.3.


RZ-021 está concluído no recorte validado. O terminal normal agora usa IRQ4/IPC e prazo para PS/2, conforme a [ADR 011](../ADR-011-TERMINAL-UART.md). O [ensaio instrumentado](../../evidencias/teste-uart-1794726895277854900/resultado.txt) passou, inclusive na janela de rearme controlada; o ensaio normal, construção, verificação de imagem e cinco boots normais também passaram. Os ensaios adicionais de IRQ, quota, vida e prazo-idle passaram; no host passaram 31 testes do núcleo e 23 dos pacotes. O PS/2 emulado por HMP executou o comando sobre e recebeu resposta distinta. As rajadas ensaiadas foram de quatro bytes a cada 50 ms. PS/2 físico e fallback com kernel antigo não foram validados em VM; não há garantia de perda zero geral. RZ-022 e RZ-023 foram concluídos apenas nos respectivos escopos registrados; RZ-024 foi concluído no recorte controlado descrito no [resumo](../../evidencias/execucao-rz-024-20261115/RESUMO.md). A matriz T03 ainda mantém lacunas, e o marco 0.3 continua aberto.
