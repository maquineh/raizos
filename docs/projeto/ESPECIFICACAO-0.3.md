# Especificação proposta — Raiz OS 0.3

**Estado:** semântica decidida na [ADR 003](../ADR-003-ESPERA-0.3.md). As [ADRs 004](../ADR-004-ESPERA-IPC-ABI-2.md) e [005](../ADR-005-RECEPCAO-BLOQUEANTE.md) acrescentam espera e recepção da caixa própria à ABI 2. A [ADR 006](../ADR-006-PRAZOS-IPC.md) adiciona prazo à recepção, exercitado no recorte de RZ-016. A ADR 010 implementa o recorte IRQ4 da ABI 2, validado no ensaio opt-in; espera multiplexada completa e ABI futura ainda não estão implementadas.

A [ADR 007](../ADR-007-ESPERA-E-CICLO-DE-VIDA.md) acrescenta observação one-shot da rede pelo supervisor durante uma recepção, exercitada no recorte de RZ-017. Ela não oferece inscrições nem notificações persistentes de RZ-018.

A [ADR 008](../ADR-008-NOTIFICACOES-VIDA.md) implementa inscrição e slot de
ciclo de vida persistentes no recorte supervisor→rede; o ensaio de vida e as regressões passaram em QEMU.
**Base preservada:** código e evidências da versão 0.2.0, com ABI de serviços 2.  
**Objetivo:** permitir que serviços esperem trabalho sem consultar continuamente filas vazias, preservando identidade de instância, autoridade e recuperação.  
**Requisitos relacionados:** RQ-009, RQ-010 e RQ-011 do PRD.  
**Trabalho relacionado:** RZ-013 a RZ-025 do backlog.

Os nomes abaixo descrevem o desenho decidido, ainda não implementado. Não atribuem números de syscalls, códigos de erro, valores de estados ou um número de ABI futura. A codificação binária será definida e testada antes de expor as operações, com atualização conjunta de kernel, wrappers e serviços. A fonte normativa atual continua sendo [`contrato-sistema`](../../contrato-sistema/src/lib.rs).

## 1. Resultado que o marco deve produzir

Um serviço sem trabalho deve poder ficar fora da fila de execução até receber mensagem, notificação autorizada ou atingir seu prazo. O kernel deve manter a máquina capaz de acordar quando todos os serviços estiverem bloqueados. A chegada de um evento antes, durante ou depois do registro da espera não pode deixá-lo dormindo indefinidamente por perda de despertar.

O mínimo proposto para 0.3 inclui recepção/espera multiplexada bloqueante. **O envio continua não bloqueante por padrão**, com erro explícito de fila cheia. Isso evita que terminal, driver e supervisor fiquem presos tentando enviar enquanto deveriam receber ou atender hardware. A alternativa de envio bloqueante está delimitada na seção 7.4 e depende de decisão adicional; não é requisito implicitamente aceito.

O primeiro piloto de interrupção de dispositivo será **recepção da UART compatível com 16550, COM1, IRQ4 exclusiva, na máquina QEMU `pc`/i440fx**. O kernel entrega uma notificação; o serviço de terminal interpreta e atende a UART. A rede e o teclado PS/2 continuam com polling, mas acionado por prazo e com orçamento de trabalho. Não se afirma que 0.3 eliminará todo polling.

Critérios de resultado:

1. Esperar uma caixa vazia retira o processo do agendamento; uma mensagem válida o acorda e é consumida uma única vez.
2. Prazo, cancelamento e reinício de uma instância não deixam registros de espera capazes de modificar seu sucessor.
3. Fila IPC cheia não impede a entrega de notificações de IRQ ou de invalidação de referência.
4. Todos os processos bloqueados levam a um contexto de ociosidade que desperta pelo timer ou pela IRQ autorizada, sem pânico do kernel.
5. O terminal recebe entrada serial por IRQ4 e mantém atendimento a IPC; rede e supervisor preservam suas responsabilidades em ring 3.
6. Os testes de isolamento, autoridade, referências e recuperação de 0.2 continuam válidos após adaptar somente as verificações de estados que o novo contrato deliberadamente alterar.


RZ-020 está concluído no recorte de IRQ4 exclusiva: a [ADR 010](../ADR-010-NOTIFICACAO-IRQ4.md) acrescenta à ABI 2 uma associação exclusiva de IRQ4 ao terminal, notificação coalescida separada da fila IPC e confirmação por sequência. No [ensaio opt-in em QEMU](../../evidencias/teste-irq-1794725963506321900/resultado.txt), IRQ4 acordou o terminal a partir de idle CPL 0; foram observadas negação a outro serviço, recusas de sequência incorreta e duplicada, rechecagem e rearme. Com B pendente e sem confirmação por 50 tiques, a máscara foi observada em duas amostras, o contador de entradas permaneceu igual e o timer avançou; C chegou após o rearme. Coalescência foi testada no host, não demonstrada nesse ensaio QEMU. Isso não demonstra ausência absoluta de tempestades nem suporte a hardware físico. A construção normal, verificação de imagem e três comandos de regressão em QEMU (cinco boots) passaram. Naquela execução o terminal normal ainda usava polling. SMP e IRQ compartilhada não foram demonstrados. Os ensaios reconstruídos de quota, vida e prazo-idle também passaram; o [resumo da execução RZ-020](../../evidencias/execucao-rz-020-20261115/RESUMO.md) reúne logs, hashes e limites.

## 2. O que a base 0.2 oferece e o que precisa mudar

| Base examinada | Comportamento existente | Consequência para 0.3 |
|---|---|---|
| [`controle.rs`](../../nucleo/src/controle.rs) | Caixa FIFO de 16 mensagens; referências com geração; purga por origem exata | Preservar ordem, identidade e limites; acrescentar estado de espera sem tornar a caixa um canal de eventos de hardware |
| [`processos.rs`](../../nucleo/src/processos.rs) | `RECEBER` consulta sem bloquear; extensão de recepção bloqueante implementada; três contextos | Preservar distinção entre vida e agendamento e ampliar fontes de espera |
| [`cpu.rs`](../../nucleo/src/cpu.rs) | PIT aproximadamente a 100 Hz, PIC, portas de interrupção, retorno a CPL 3 | Criar entrada e retorno válidos para IRQ ocorrida durante o contexto ocioso em CPL 0 |
| [`processos.rs`](../../nucleo/src/processos.rs) | Idle em CPL 0 e espera de disponibilidade da própria caixa, com despertar por IPC admitido | Integrar espera completa aos serviços e validar os cenários de geração, recuperação, prazo e IRQ |
| [`servico-rede`](../../servicos/rede/src/main.rs) | Aguarda IPC por até 10 tiques; processa até 16 quadros por rodada e cede se o orçamento de RX se esgotar; reset usa esperas limitadas por tiques | Manter polling residual da e1000 e validar saturação, fallback e regressões |
| [`servico-supervisor`](../../servicos/supervisor/src/main.rs) | Inscrição IPC/vida; no automático, rede saudável espera sem prazo e falha espera o restante do backoff de 100/200/400 tiques | Repetir execução normal e complementos de integração do RZ-022 |
| [`servico-terminal`](../../servicos/terminal/src/main.rs) | Espera IRQ4/IPC e usa prazo para polling PS/2 | Validar integração e regressões segundo a [ADR 011](../ADR-011-TERMINAL-UART.md) |

O [resumo de validação 0.2](../../evidencias/RESUMO-VALIDACAO.md) registra 26 testes no host e cinco inicializações de integração em QEMU aprovadas. As evidências cobrem preempção, uma violação de memória provocada, referências antigas e recuperação limitada da rede. **Não cobrem as esperas, deadlines, IRQ4 ou idle propostos aqui.** Um [ensaio posterior de idle](../../evidencias/execucao-rz-014-20261115/final/RELATORIO-TESTE-IDLE.md) foi aprovado em QEMU com despertar provocado pela feature de teste, sem demonstrar evento real de serviço.

Um ensaio em imagem opt-in exercitou espera de disponibilidade e envio IPC real entre terminal e supervisor em CPL 3. Essa operação é agora extensão aditiva ABI 2, conforme ADR 004; sua integração normal foi reconstruída e passou pelas regressões registradas no [resumo da execução](../../evidencias/execucao-supervisor-20261115/RESUMO.md). A mensagem permanece na FIFO até `RECEBER`; a purga pode esvaziar a fila antes da execução do receptor. O ensaio anterior empregava chamada privada; o ensaio posterior usou a extensão pública. O registro pertence ao processo e é cancelado ao terminar ou substituir a instância, mas não identifica callbacks atrasados por geração ou sequência. O recorte da ADR 004 não inclui prazo, notificação de IRQ ou cancelamento remoto de espera. RZ-015 foi concluído para recepção da própria caixa, com [evidências específicas](../../evidencias/execucao-rz-015-20261115/RESUMO.md); a revogação entre despertar e seleção foi ensaiada no host, não em VM.

Também existe uma distinção importante no alcance dos testes: a purga de filas ocupadas foi exercitada no módulo de controle no host; nos reinícios observados na VM, o número de mensagens pendentes era zero. A integração 0.3 deve incluir filas ocupadas durante bloqueio e recuperação, sem apresentar o resultado anterior como prova dessa nova interação.

A [execução RZ-016](../../evidencias/execucao-rz-016-20261115/RESUMO.md) exercitou timeout em QEMU inclusive com três serviços bloqueados e idle em CPL 0. Empate evento/timeout, volta do contador e substituição de instância foram testados no host, não em VM. A compatibilidade com kernel antigo não foi testada em VM. Esse ensaio anterior não cobriu a extensão posterior de IPC/vida; não conclui a espera multiplexada completa nem o marco 0.3.

A [execução RZ-017](../../evidencias/execucao-rz-017-20261115/RESUMO.md) exercitou em QEMU cancelamento por encerramento/reinício, preservação do prazo após recusa por quota e observação pontual da falha da rede. Falha de preparação e concorrência entre timeout e invalidação foram testadas no host; observação durante substituição e fallback com kernel anterior não foram ensaiados em VM. A observação não é persistente e não conclui RZ-018.

RZ-019 está concluído no recorte implementado conforme a [ADR 009](../ADR-009-LIMITES-IPC.md): caixas de 16 mensagens, limite de 15 por origem lógica na caixa do terminal e slots estruturais de inscrição e espera por processo. Passaram 29 testes do núcleo no host; as regressões normais em QEMU passaram. RZ-020 está concluído no recorte de IRQ4 exclusiva. O [ensaio de quota em QEMU](../../evidencias/teste-quota-1794725073649578100/resultado.txt) passou para 15 envios da rede, recusa do 16º, reserva ao supervisor, FIFO e readmissão após drenagem. A ordem inversa e liberação após uma retirada ou purga foram testadas no host, não nesse ensaio de VM. O [resumo da execução](../../evidencias/execucao-rz-019-20261115/RESUMO.md) reúne construção normal, verificação de imagem e os cinco boots de regressão.

O [ensaio RZ-018](../../evidencias/teste-vida-1794724542137348500/resultado.txt) passou em QEMU com autorização, FIFO saturada, transições e invalidação por geração no recorte supervisor→rede; as regressões também passaram. Uma tentativa inicial da fixture falhou e foi corrigida, com histórico preservado. Os cinco ensaios específicos anteriores reconstruídos passaram, conforme o [resumo da execução](../../evidencias/execucao-rz-018-20261115/RESUMO.md); as regressões gerais também passaram novamente na imagem normal final.

O RZ-022 foi concluído no escopo validado. A [ADR 012](../ADR-012-ESPERA-SERVICOS-RZ022.md) e o [resumo da execução](../../evidencias/execucao-rz-022-20261115/RESUMO.md) registram integração final, ciclo de espera, regressões e limites. O RZ-023 foi concluído no escopo do aceite original com 12 comandos de regressão após rebuild, testes host e cinco boots normais; consulte o [resumo](../../evidencias/execucao-rz-023-20261115/RESUMO.md) e a [matriz T03](../../evidencias/execucao-rz-023-20261115/MATRIZ.md). RZ-024 foi concluído no recorte controlado de IRQ4, prazo e pressão finita; o [resumo](../../evidencias/execucao-rz-024-20261115/RESUMO.md) registra contagens e limites. A matriz mantém lacunas por cenário; nem todos os T03 estão demonstrados e o marco 0.3 continua pendente.

## 3. Escopo e condições de validade

O desenho inicial conserva uma CPU, três serviços, uma thread por serviço, memória e filas limitadas e imagem construída como produto único. As regiões críticas do estado de processos continuam serializadas com interrupções mascaráveis desabilitadas na CPU única. Handlers não devem reentrar nesse estado enquanto a transição está incompleta.

Isso não constitui protocolo de sincronização para SMP. Antes de ativar outra CPU, será necessário rever propriedade dos contextos, locks, ordenação de memória, afinidade de IRQs e troca de mensagens entre CPUs. Desabilitar interrupções localmente não serializaria outra CPU.

Ficam fora deste marco: criação geral de threads, prioridades de tempo real, herança de prioridade, timer sem ticks periódicos, roteamento PCI INTx genérico, interrupções compartilhadas arbitrárias, MSI/MSI-X, IOMMU, TCP e recuperação persistente de aplicações. O piloto não acrescenta confiança em drivers DMA hostis.

## 4. Modelo de vida, agendamento e espera

### 4.1 Separar duas perguntas

O estado de vida informa se a instância está ativa, falhou ou terminou. O estado de agendamento informa se uma instância ativa está executando, pronta ou bloqueada. **Dormir não é falhar.**

Essa separação evita duas regressões concretas: recusar mensagens porque o destino está esperando e fazer o supervisor reiniciar um serviço saudável que está bloqueado. As verificações de `ENVIAR`, `ENCERRAR`, `REINICIAR` e do supervisor devem usar o estado apropriado, em vez de espalhar comparações com o antigo `EXECUTAVEL`.

| Situação conceitual | Receber nova mensagem | Ser escolhido pelo escalonador | Poder ser encerrado/reiniciado pelo supervisor autorizado |
|---|---|---|---|
| Ativo e pronto/executando | Sim | Sim | Sim, para o alvo permitido |
| Ativo e bloqueado | Sim; pode provocar despertar | Após conclusão da espera | Sim; cancelar e desvincular a espera primeiro |
| Falhou/finalizado | Recusa de serviço indisponível | Não | Reinício conforme autoridade e quota |
| Referência de geração anterior | Recusa por referência antiga | Nunca | Recusa por referência antiga |

A consulta de vida distinguirá ativo, falhou e finalizado; bloqueio é condição de agendamento, não falha. Uma consulta de agendamento, se necessária, será separada. Não reutilizar silenciosamente um valor ABI 2 com significado incompatível.

### 4.2 Registro mínimo de espera

Cada serviço pode ter, no máximo, uma espera bloqueante em andamento. O registro pertence ao kernel e contém, conceitualmente:

- Referência completa da instância que espera.
- Sequência de operação de espera, diferente a cada registro.
- Conjunto limitado de fontes autorizadas: caixa IPC própria, IRQ atribuída, observação de ciclo de vida permitida e/ou prazo.
- Referências exatas dos pares observados; nunca apenas o ID lógico.
- Prazo opcional e resultado ainda não concluído.
- Ligações aos índices de espera necessários para remoção em tempo limitado.

Uma conclusão só pode atingir o registro se **referência e sequência da operação** coincidirem. Usar apenas o índice do processo permitiria que um timeout antigo acordasse a nova rede. A política para esgotamento do contador deve recusar novo registro ou exigir reinicialização controlada; não pode dar a volta e reutilizar silenciosamente uma identidade pendente.

As estruturas podem ser estáticas nesta etapa. Não é necessário introduzir um alocador geral para três registros de espera, filas limitadas e associações de IRQ fixas.

### 4.3 Operações decididas, sem codificação binária aprovada

| Operação conceitual | Entrada | Resultado esperado |
|---|---|---|
| Tentar receber | Caixa própria | Mensagem ou ausência imediata, preservando a semântica atual |
| Esperar evento | Fontes autorizadas e prazo | Uma mensagem consumida, notificação de IRQ, mudança/invalidação observada ou prazo esgotado |
| Receber com prazo | Conveniência sobre espera de IPC | Mensagem, timeout ou erro; sem laço de consultas vazias no serviço |
| Dormir até prazo | Prazo finito | Despertar por expiração ou cancelamento de ciclo de vida |
| Observar instância | Referência exata e interesse autorizado | Inscrição ou estado já observado, sem janela entre consultar e inscrever |
| Concluir/rearmar IRQ | Associação atribuída e sequência entregue | Rearme autorizado ou erro por associação/geração/sequência inválida |

A espera multiplexada deve ser a primitiva utilizada pelos serviços com várias responsabilidades. Fazer o terminal esperar apenas uma resposta de ping bloquearia sua leitura de teclado e suas mensagens do supervisor.

O retorno deve ser discriminado por classe de evento definida pelo kernel. Uma mensagem de usuário com um código parecido não pode virar notificação privilegiada. Não usar uma `Mensagem` com origem zero como atalho. Layout e registradores serão definidos e testados no crate de contrato antes da exposição; RZ-013 decide semântica e compatibilidade, não a codificação binária.

## 5. Registro atômico e prevenção de perda de despertar

### 5.1 Caminho recomendado

Na entrada da operação de espera, com o estado do kernel serializado:

1. Validar chamador, referências, fontes autorizadas e prazo; processar expirações vencidas conforme a regra definida na seção 6.
2. Procurar um evento elegível já pendente. Se existir, consumi-lo/registrar sua entrega e retornar sem bloquear.
3. Se não existir, registrar fonte, referência, sequência e prazo.
4. Marcar a instância como bloqueada antes de abandonar a região crítica.
5. Escolher outro serviço pronto ou o contexto ocioso.

Quem publica uma mensagem ou notificação deve, na mesma região crítica, atualizar o estado pendente e tentar concluir a espera compatível. A conclusão grava o resultado no contexto salvo, remove as demais inscrições daquela operação e torna o processo pronto. Ela não executa código de usuário dentro do handler.

Proposta inicial para mensagens: sempre usar a mesma operação FIFO para inserção e retirada, inclusive quando há receptor bloqueado. O caminho de despertar insere e seleciona a mensagem no kernel, evitando uma segunda implementação de “entrega direta” com regras diferentes de ordem e validação.

### 5.2 Interleavings obrigatórios

| Evento chega… | Comportamento necessário |
|---|---|
| Antes de entrar na espera | A verificação inicial encontra o evento e não dorme |
| Entre a verificação e a publicação da espera | A região crítica impede esse interleaving de modificações do kernel na CPU única; IRQ pendente é atendida depois da publicação |
| Depois de publicar a espera e antes de trocar contexto | Publicador encontra o registro e o conclui; processo fica pronto |
| Enquanto a CPU está ociosa | Handler reconhece o contexto idle, publica o evento e permite retornar a um serviço pronto |
| Depois de timeout/cancelamento | Registro antigo não corresponde mais ao token; mensagem válida fica disponível para nova recepção, sem completar a chamada antiga |

`CLI` não impede o dispositivo de produzir dados, nem substitui o tratamento de IRQ pendente. A atomicidade protege estruturas do kernel; o protocolo de atender/rearmar o dispositivo é separado, descrito na seção 10.

### 5.3 Uma conclusão por operação

Mensagem, deadline, revogação e IRQ podem disputar a mesma espera. Apenas a primeira conclusão válida pode mudar o resultado da operação. As demais fontes permanecem pendentes quando aplicável. Remover inscrições de uma espera concluída não autoriza apagar outra mensagem, zerar todas as notificações ou reutilizar seu token.

Uma IRQ pode ser agrupada com outras ocorrências. Uma mensagem IPC não: deve preservar seu conteúdo, origem e ordem, ou ter sua recusa/purga explicitamente definida.

## 6. Prazos, cancelamento e semântica de conclusão

### 6.1 Relógio e expiração

Usar o contador monotônico de tiques do kernel, não horário de calendário. O PIT atual fornece granularidade aproximada de 10 ms; isso não é garantia de latência máxima de retorno. Períodos longos com IRQs desabilitadas, atendimento de outros serviços e execução no emulador afetam a observação do prazo. [1]

A API distingue tentativa imediata, prazo finito e ausência explícita de prazo. Tentativa imediata verifica eventos pendentes e retorna ausência sem bloquear; não é um prazo já vencido. Prazo finito é duração relativa de 1 a 2^63−1 tiques, somada modularmente ao contador de 64 bits na admissão; valores maiores são recusados. Zero representa tentativa imediata e ausência de prazo é opção distinta. `vencido = agora.wrapping_sub(prazo) < 2^63`; a comparação exige processar a expiração antes de ela estar vencida por 2^63 tiques observados. Tiques observados não medem tempo físico enquanto IRQ estiver mascarada.

Regra de desempate: antes de concluir uma operação com prazo finito, verificar a expiração pelo relógio lógico observado sob a mesma exclusão. Se o prazo já venceu, concluir timeout sem consumir o evento em seu nome. Uma conclusão registrada antes do vencimento continua válida, mesmo que o serviço só volte a executar depois. Essa ordem precisa ser exercitada em testes; não prometer ordenação por nanossegundo do host.

Para três processos, uma varredura limitada dos registros de prazo a cada IRQ0 é suficiente como implementação inicial. Uma heap de timers não deve ser introduzida sem necessidade medida. Remoção por token e geração continua obrigatória em qualquer estrutura.

### 6.2 O que timeout significa

Timeout de recepção significa que aquela espera terminou sem consumir uma mensagem para ela. Não cancela um pedido que já foi enviado e aceito pelo servidor. A mensagem de resposta pode chegar depois e deve ser correlacionada ou descartada pela aplicação de forma explícita.

Sucesso de `ENVIAR` confirma aceitação na caixa, não execução da operação, persistência ou resposta. Reinício pode purgar uma mensagem aceita que ainda não foi consumida. Se uma aplicação precisar distinguir aceito, iniciado e concluído, deverá definir identificador de operação e confirmações no protocolo; o kernel não pode inferir essas etapas.

### 6.3 Cancelamento suportável no mínimo proposto

São obrigatórios cancelamento/desvinculação ao encerrar ou substituir a instância e conclusão por invalidação de uma referência inscrita como fonte da espera. O processo antigo nunca retorna após ser substituído. Se outro resultado vencer primeiro, a invalidação permanece pendente para a próxima espera.

Não prometer que uma única thread já bloqueada executará uma chamada para cancelar a si própria. No terminal, `Ctrl+C` exige que a entrada faça parte da espera multiplexada e que pedidos longos sejam operações de aplicação pendentes, sem bloquear todo o serviço em um envio. Cancelamento remoto genérico de espera exigiria autoridade nova e fica sujeito a ADR; conhecer um token não deve conceder esse direito.

Uma interrupção não deve cancelar arbitrariamente uma operação aceita. Ela é uma fonte de despertar autorizada. Não há promessa de desfazer efeitos de mensagens já consumidas pelo destinatário.

## 7. Filas cheias, justiça e progresso

### 7.1 Preservar contrapressão explícita

O mínimo recomendado mantém `ENVIAR` não bloqueante: sem espaço, retornar fila cheia sem sobrescrever mensagens e sem inserir parcialmente. Serviços guardam, quando necessário, uma saída pendente em buffer próprio limitado e continuam atendendo suas outras fontes. A retentativa tem prazo e orçamento; esgotamento deve ser observável. Não substituir os atuais laços de até 64 tentativas por espera infinita disfarçada.

A caixa de 16 posições é a referência inicial de avaliação, não uma justificativa para crescimento ilimitado. Notificações de kernel têm armazenamento separado e limitado: um slot/estado pendente por fonte atribuída ou observação autorizada. Fila de usuário cheia não pode esconder a morte de um alvo nem perder o único aviso de uma IRQ mascarada.

### 7.2 Admissão e consumo justos

O terminal recebe de rede e supervisor. Cada uma dessas origens pode ocupar até 15 das 16 posições da caixa, deixando uma posição disponível para a outra. Nas demais caixas, a capacidade total permanece 16. Exceder o limite de origem retorna fila cheia; não há fila adicional.

Manter FIFO das mensagens admitidas. Entre IPC, IRQ e ciclo de vida, selecionar classes em rotação, avançando após cada entrega e ignorando classes sem evento elegível. Isso não promete progresso se o serviço não executar ou não consumir eventos.

O round-robin continua entre processos prontos. Uma IRQ não deve executar o driver inteiro no kernel ou conceder uma sequência ilimitada de quanta ao proprietário. Justiça pressupõe handlers e regiões críticas limitados; não equivale a garantia de tempo real.

### 7.3 Agrupamento de notificações

Uma notificação de IRQ significa “há trabalho/condição para inspecionar”, não “exatamente um byte ou pacote”. Agrupar ocorrências exige um indicador pendente e uma sequência de entrega/rearme; contadores de diagnóstico podem saturar de forma explícita. Mensagens não devem ser fundidas dessa maneira.

Uma observação de ciclo de vida é relativa a uma referência exata. Se várias mudanças forem condensadas, preservar a informação de invalidação e declarar que a observação fornece estado relevante mais recente, não um histórico completo de todas as transições. Logs de auditoria seriam outra responsabilidade.

### 7.4 Alternativa de envio bloqueante — não adotada por este texto

Uma decisão futura para acrescentá-la exigiria, no mínimo, copiar a mensagem para um registro do kernel antes de bloquear; guardar remetente, destino e geração exatos; exigir prazo para envios que possam formar ciclos; manter ordem FIFO dos remetentes esperando espaço; e impedir que envios novos ultrapassem indefinidamente quem já espera.

Ao liberar uma posição, admitir atomicamente o primeiro envio válido, concluir seu remetente e retirar seu registro. Expiração, morte e reinício devem remover o envio antes que a posição seja ocupada por ele. Depois de admitido, não é mais possível prometer retirada por timeout do remetente. Ciclos de serviços esperando espaço continuam possíveis; prazos e desenho dos protocolos são necessários. Essa alternativa aumenta o escopo de estados e testes e não deve ser acrescentada sem revisar o backlog.

## 8. Integração com geração, falha e reinício

Preservar a ordem já adotada na 0.2: validar autoridade e referência, verificar quota, preparar imagem/tabelas em arena nova e somente depois revogar a instância antiga. Uma recusa de quota ou preparação deve preservar também suas **esperas, inscrições de IRQ, prazos e mensagens**, além do contexto e DMA. [2]

Após preparação bem-sucedida, a revogação proposta acrescenta, na mesma transição serializada:

1. Impedir novos eventos de hardware de serem atribuídos à geração antiga, mascarando a fonte quando houver associação.
2. Desativar bus mastering conforme a política atual de dispositivo; conservar a arena antiga em quarentena.
3. Retirar o processo do agendamento e desvincular sua operação de espera e seus prazos.
4. Invalidar associações e sequências de IRQ pertencentes à geração antiga.
5. Cancelar dependências de outros processos nessa referência exata e produzir notificações reservadas.
6. Purgar mensagens da origem revogada sem apagar mensagens de outras origens; esvaziar a caixa destinada à antiga instância.
7. Publicar contexto, referência e associações da nova instância, ainda com dispositivo desarmado até sua inicialização.

Uma operação que aponta para a geração antiga nunca é redirecionada automaticamente. O supervisor deve observar explicitamente a nova referência devolvida pelo reinício. O retorno de reinício continua distinto da confirmação posterior de driver pronto.

A inscrição autorizada combina consulta e registro sob a mesma exclusão, com um slot por par observador e alvo lógico autorizado, ligado à referência exata. No recorte inicial, apenas o supervisor pode observar a rede. Retorna o estado atual e registra interesse persistentemente; estado já falho ou finalizado não produz também notificação duplicada da mesma transição. Mudança posterior ocupa o slot pendente. Referência já substituída retorna referência antiga sem inscrever. Cancelamento ou revogação do observador libera o slot; revogação do alvo publica invalidação terminal até consumo ou cancelamento e então libera o slot, sem redirecionar à nova geração. Enquanto a invalidação ocupar o slot, inscrever a geração nova retorna recursos esgotados.

## 9. Ociosidade quando todos esperam

Introduzir um contexto ocioso próprio, com pilha, identificação e caminho de IRQ definidos. Não representá-lo fingindo que o último processo continua ativo. O despachante deve distinguir interrupção de um serviço, interrupção do contexto ocioso e falha real ocorrida em código de kernel.

O ciclo de seleção verifica trabalho com IRQs mascaráveis desabilitadas. Sem trabalho, pode usar a sequência arquitetural adequada de habilitar interrupções e suspender a CPU, como `sti; hlt`, preservando a janela de proteção exigida por x86. Não colocar instruções arbitrárias entre habilitar e suspender; validar as condições dessa sequência, o frame de CPL 0 e o retorno em código e teste. A semântica de `STI` consta no manual Intel. [3]

O IRQ0 do PIT deve continuar incrementando o relógio e expirando esperas durante idle. IRQ4 deve publicar sua notificação mesmo sem processo de usuário em execução. O handler não pode gravar um frame ocioso como contexto da rede, aplicar ao idle a regra atual que encerra o kernel em toda entrada de CPL 0, nem voltar com pilha/seletor incorretos.

Se não há serviço vivo, definir uma parada diagnóstica distinta da ociosidade normal. Se há serviços vivos bloqueados sem nenhuma fonte possível de despertar, registrar a condição; não reanimar processos arbitrariamente para esconder deadlock. O teste de idle usará um cenário controlado com todos bloqueados e uma IRQ ou prazo conhecido, não uma promessa de ausência de atividade periódica na operação normal.

## 10. IRQ: fronteira de autoridade e piloto COM1

### 10.1 Papel do kernel

O kernel mantém a associação de uma fonte concreta à referência autorizada, estado de máscara, sequência de entrega e pendência. Somente ele controla PIC/IDT, EOI e autorização de rearme. O serviço não recebe permissão para mascarar IRQ0, reconfigurar todas as linhas ou reconhecer interrupção pertencente a outro serviço.

O handler da fonte deve executar trabalho limitado: identificar a linha, aplicar a estratégia de máscara, registrar pendência, concluir espera elegível e reconhecer o controlador de interrupções na ordem necessária. A interpretação dos registradores UART permanece no terminal. Reconhecer o PIC não é o mesmo que remover a causa no dispositivo.

`concluir/rearmar` exige associação e sequência atuais. Confirmação duplicada, associação inexistente, chamador errado ou geração revogada deve ser recusada sem desmascarar uma fonte. O token de associação também não é uma capability delegável: a autoridade real continua sendo conferida pelo kernel.

### 10.2 Plataforma de ensaio proposta

| Item | Perfil inicial |
|---|---|
| Máquina | QEMU `pc`/i440fx, TCG, uma CPU, 256 MiB, OVMF; manter o perfil já usado na 0.2 |
| Timer | PIT existente, IRQ0, aproximadamente 100 Hz |
| Piloto | UART compatível com 16550, COM1 em `0x3f8`, IRQ4 no PIC mestre, sem compartilhamento no ensaio |
| Proprietário | Referência atual do terminal |
| Estímulo | Bytes enviados pelo backend serial do harness, em instantes controlados |
| Rede | e1000 continua com INTx desabilitado e polling por prazo |

O mapa final do dispositivo e seu roteamento devem ser conferidos no ensaio. Esta seleção não certifica uma UART física nem torna intercambiáveis plataformas com APIC, MSI ou linhas compartilhadas.

### 10.3 Atendimento e rearme sem perda

Inicializar com fonte mascarada. O terminal configura a UART, trata estado pendente e só então solicita armação. No piloto, usar recepção e suas causas necessárias; não habilitar fontes de transmissão/modem sem handler correspondente. Os registradores IER/IIR/LSR e o timeout do FIFO têm papéis diferentes: IIR identifica causa, LSR informa dados/erros e a leitura de recepção drena bytes. [4]

Após notificação, o terminal drena trabalho com limite por rodada. Se o limite for atingido com trabalho restante, mantém a pendência local e cede; não dorme esperando uma nova borda que talvez não aconteça. Ao esvaziar as causas, confirma/rearma a associação e verifica novamente a condição do dispositivo antes de bloquear. Dados surgidos na janela de rearme precisam ser vistos pelo estado local ou pela notificação persistente do kernel.

A ordem máscara → pendência → EOI → atendimento → rearme precisa de testes específicos para a UART/PIC selecionada. Não assumir que uma linha que permaneceu afirmada produzirá outra borda apenas porque houve EOI. Também não exigir uma IRQ por byte: FIFO e agrupamento tornam essa equivalência incorreta.

Se o serviço morre com a fonte mascarada, ela fica desarmada; não rearmar para um proprietário inexistente. Se o serviço se recusa a confirmar, as outras fontes e o timer devem continuar funcionando. Uma política de quarentena de fonte ruidosa pode limitar interrupções, mas seu limiar e sua recuperação precisam ser declarados, sem transformar IRQ0 em vítima desse controle.

**A IRQ da e1000 não faz parte do primeiro critério de aceite.** Acrescentá-la exige demonstrar roteamento da linha PCI, modo elétrico/controle apropriado, compartilhamento, leitura/limpeza de causa e rearme sob RX/TX concorrente. Habilitar o bit PCI que libera INTx sem essa cadeia não é uma implementação de notificações de rede. [5]

## 11. Alterações esperadas nos serviços e polling residual

| Componente | Mudança proposta | Trabalho que continua por consulta |
|---|---|---|
| Terminal | Esperar IPC, IRQ de recepção serial e próximo prazo de PS/2; manter pedidos longos como estado de aplicação | PS/2; envio serial inicialmente pode manter espera limitada por registrador, com contador de falhas e impacto medido |
| Rede | Aguardar IPC ou próximo prazo de RX/TX; manter orçamento de 16 quadros como referência inicial; trocar espera ociosa de reset por prazo bloqueante | Anéis e1000, conclusão de TX e verificação pontual do reset; não haverá IRQ da NIC no piloto |
| Supervisor | Observar ciclo de vida por referência; aguardar comandos ou deadline de backoff | Consulta pontual para reconciliar notificação/estado e após nova inscrição; não consulta contínua de serviço saudável |

Se a rede alcançar o orçamento e ainda houver quadros, deve continuar elegível para outra rodada após ceder, sem dormir até o próximo tique por engano. O prazo de polling em repouso deve ser um parâmetro registrado; reduzir frequência pode aumentar latência e perdas em rajadas. A versão atual não oferece controle de fluxo capaz de prometer ausência de perdas arbitrárias.

O supervisor conserva modo manual inicial, orçamento de recuperação e limite de arenas da 0.2. Receber uma notificação de morte não autoriza reinício em modo manual. Continuar vivo mas sem progresso também não passa a ser detectável apenas porque existe IPC bloqueante.

## 12. Plano de implementação por unidade revisável

| Trabalho | Entrega proposta | Dependência |
|---|---|---|
| RZ-013 | ADR de semântica, resultados, compatibilidade e limites; codificação e testes de layout precedem a exposição das operações | Revisão desta especificação |
| RZ-014 | Estados de vida/agendamento, registros de espera e contexto idle válido | RZ-013 |
| RZ-015 | Recepção bloqueante e publicação/despertar atômicos | RZ-014 |
| RZ-016 | Prazos, comparação temporal, expiração limitada | RZ-014 e RZ-015 |
| RZ-017 | Cancelamento e limpeza por geração, morte e reinício | RZ-015 e RZ-016 |
| RZ-018 | Observação atômica de ciclo de vida e notificações reservadas | RZ-017 |
| RZ-019 | Quotas, fila cheia, admissão entre origens e limites de notificações | RZ-013 a RZ-018 |
| RZ-020 | Associações de IRQ, autoridade, sequência e rearme | RZ-014, RZ-017 e RZ-019 |
| RZ-021 | UART COM1/IRQ4, harness de estímulo e janela de rearme | RZ-020 |
| RZ-022 | Migração dos três serviços, mantendo polling residual declarado | RZ-015 a RZ-021 |
| RZ-023 | Testes de interleavings e integração, incluindo regressões 0.2 | Incremental em cada unidade; gate final após RZ-022 |
| RZ-024 | Contadores de espera, despertar, timeout, rejeição e idle | Desde RZ-014; coleta final após RZ-022 |
| RZ-025 | Relatório, documentação ABI aprovada e imagem do marco | Todos os gates anteriores |

Impactos principais previstos: `contrato-sistema` e `apoio-servicos` para contrato/wrappers; `nucleo::controle` para modelos testáveis; `processos` para transições e ciclo de vida; `cpu`/`plataforma` para contexto idle e IRQs; `servicos/*` para laços de eventos; `ferramentas` para estímulos e critérios observáveis. Os nomes de novos módulos ficam a cargo da implementação. Não duplicar no driver uma política de autorização já pertencente ao kernel.

## 13. Matriz de testes futuros — nenhum executado neste documento

**Os casos T03 abaixo são critérios planejados; a matriz de RZ-023 inventariou evidências parciais e pendências, mas não demonstra cobertura integral de cada cenário.** Alguns comportamentos foram exercitados nos incrementos RZ-014 a RZ-023. A coluna descreve o que deverá ser observado, não um resultado obtido por este documento. Os testes no host devem explorar ordenações controladas do mesmo modelo utilizado pelo kernel; a integração deve provar os caminhos de CPU, timer, contexto e dispositivo que um modelo puro não cobre.

| Caso | Cenário | Critério proposto |
|---|---|---|
| T03-01 | Mensagem já na fila ao esperar | Retorno imediato com uma mensagem; nenhum bloqueio residual |
| T03-02 | Evento nas fronteiras de testar/registrar/agendar | Todas as ordenações válidas concluem ou conservam pendência; nenhum despertar perdido |
| T03-03 | Receptor bloqueado e remetente legítimo | Mensagem entregue uma vez, origem exata e FIFO preservados |
| T03-04 | Destino bloqueado, mas vivo | Envio aceito conforme quota; supervisor não o trata como falho |
| T03-05 | Prazo e evento no mesmo tique lógico | Resultado único conforme regra definida; evento não consumido indevidamente após timeout |
| T03-06 | Rollover e prazo fora do horizonte permitido | Comparação correta ou recusa explícita, sem dormir por intervalo ambíguo |
| T03-07 | Token de timeout de uma espera já cancelada | Não modifica novo registro nem contexto de outra geração |
| T03-08 | Rede reiniciada enquanto há dependências/esperas | Geração antiga invalidada; sucessora começa sem registros herdados |
| T03-09 | Quota de reinício esgotada com alvo bloqueado | Instância, espera, dispositivo e filas anteriores preservados |
| T03-10 | Falha na preparação da imagem nova | Nenhuma revogação da instância anterior; registro de erro observável |
| T03-11 | Caixa cheia e notificação de morte/IRQ | Notificação reservada permanece entregável; nenhuma mensagem de outra origem apagada |
| T03-12 | Rede envia continuamente e supervisor publica controle | Progresso das duas origens sob quotas/admissão aprovadas |
| T03-13 | Mensagem de usuário imita código de notificação | Continua sendo IPC de usuário; não ganha autoridade nem reconhece IRQ |
| T03-14 | Todos os serviços dormem até deadline | Entrada em idle observada; IRQ0 acorda o serviço; sem `FALHA_NUCLEO` |
| T03-15 | Byte serial chega com CPU ociosa | IRQ4 registrada, terminal acorda e processa byte sem depender de polling de recepção |
| T03-16 | Byte único abaixo do limiar FIFO e rajadas | Causas necessárias atendidas; perda/overrun medidos e relatados, não ocultados |
| T03-17 | Chegada na janela de atendimento/rearme | Dado fica pendente localmente ou gera notificação; não fica abandonado até outro byte |
| T03-18 | Confirmação IRQ duplicada, errada ou sem autoridade | Recusa sem liberar fonte indevida ou afetar timer |
| T03-19 | Proprietário morre com IRQ mascarada | Fonte não é rearmada para instância inválida; demais serviços continuam |
| T03-20 | Associação/token antigo após revogação | Não confirma IRQ da geração nova; modelo cobre a revogação, integração deve declarar qual associação real foi exercitada |
| T03-21 | Fluxo contínuo de IRQ com IPC pendente | Orçamentos preservam atendimento de mensagens e preempção |
| T03-22 | Rede alcança orçamento de RX | Cede e continua o trabalho pendente; não perde progresso por dormir antecipadamente |
| T03-23 | Supervisor manual e falha observada | Notifica/observa sem reiniciar; modo automático só age após backoff previsto |
| T03-24 | Cancelamento de aplicação após envio aceito | Não afirma desfazer efeito do servidor; resposta tardia é correlacionada |
| T03-25 | Regressões de boot, ICMP/UDP, UD2, MMU, referência e quota | Critérios anteriores continuam passando, com diferenças de contrato documentadas |

Testes adicionais de ciclos de envio bloqueante só se tornam obrigatórios se essa alternativa for aceita. Não declarar sua cobertura com base nos testes de recepção.

## 14. Evidência, observabilidade e fechamento

Registrar contadores por serviço e fonte: bloqueios, despertares por causa, timeouts, invalidações, recusas de fila cheia, mensagens purgadas, IRQs registradas/agrupadas/rearmadas, confirmações recusadas e entradas em idle. Contadores devem ser limitados ou saturantes e não expor memória privada como diagnóstico.

O harness deve correlacionar operações com referência e sequência, injetar estímulos fora do código de eco do terminal e exigir marcadores de resultado distintos dos comandos digitados. Cada espera do harness precisa de timeout no host para encerrar uma VM que perdeu progresso.

Comparar o número de consultas vazias e execuções sem trabalho com a base 0.2 no mesmo perfil. Não inventar uma taxa de economia de CPU: medir quando o marco existir. O critério mínimo de idle é funcional e observável em cenário controlado; a operação normal ainda terá PIT, PS/2 e rede periódicos.

O fechamento exige código, imagem, comandos de reprodução, logs e relatório que identifique quais T03 foram executados, quais falharam e quais permanecem pendentes. Nenhuma assinatura de aprovação deste plano substitui esses resultados.

## Referências e evidências

1. [Código atual do timer e fronteiras de CPU](../../nucleo/src/cpu.rs) e [ambiente observado em 0.2](../../evidencias/RESUMO-VALIDACAO.md).
2. [ADR 002 — ciclo de vida](../ADR-002-CICLO-DE-VIDA.md), [ABI 0.2](../ABI-0.2.md) e [relatório de recuperação](../../evidencias/RELATORIO-RECUPERACAO.md).
3. Intel, *Software Developer's Manual*, volume 2B, instrução `STI`; referência arquitetural para a habilitação de interrupções. <https://www.intel.com/content/dam/www/public/us/en/documents/manuals/64-ia-32-architectures-software-developer-vol-2b-manual.pdf>
4. Texas Instruments, *PC16550D Universal Asynchronous Receiver/Transmitter With FIFOs*, SNLS378C, §§8.4 e 8.6. Cópia do documento do fabricante hospedada por distribuidor: <https://media.digikey.com/pdf/Data%20Sheets/Texas%20Instruments%20PDFs/PC16550D.pdf>
5. Intel, *PCI/PCI-X Family of Gigabit Ethernet Controllers Software Developer's Manual*, revisão 4.0; registradores de interrupção e inicialização/reset. <https://www.intel.com/content/dam/doc/manual/pci-pci-x-family-gbe-controllers-software-dev-manual.pdf>

As fontes de hardware fundamentam restrições e protocolos a verificar. Não certificam a futura implementação Raiz OS. O modelo de confiança permanece descrito em [Ameaças e limites](AMEACAS-E-LIMITES.md).


RZ-021 está concluído no recorte validado. O terminal normal agora usa IRQ4/IPC e prazo para PS/2, conforme a [ADR 011](../ADR-011-TERMINAL-UART.md). O [ensaio instrumentado](../../evidencias/teste-uart-1794726895277854900/resultado.txt) passou, inclusive na janela de rearme controlada; o ensaio normal, construção, verificação de imagem e cinco boots normais também passaram. Os ensaios adicionais de IRQ, quota, vida e prazo-idle passaram; no host passaram 31 testes do núcleo e 23 dos pacotes. O PS/2 emulado por HMP executou o comando sobre e recebeu resposta distinta. As rajadas ensaiadas foram de quatro bytes a cada 50 ms. PS/2 físico e fallback com kernel antigo não foram validados em VM; não há garantia de perda zero geral. RZ-022 e RZ-023 foram concluídos no escopo registrado em seus resumos, e RZ-024 no recorte controlado descrito no [resumo](../../evidencias/execucao-rz-024-20261115/RESUMO.md). A [matriz RZ-023](../../evidencias/execucao-rz-023-20261115/MATRIZ.md) ainda lista lacunas por cenário; a cobertura integral dos T03 e o marco 0.3 permanecem pendentes.
