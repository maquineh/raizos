# Roadmap — Raiz OS

**Base:** 0.2.0 implementada e com evidências registradas.  
**Próximo marco:** 0.3 — IPC bloqueante, timeouts, notificações de IRQ e quotas.  
**Requisitos:** [PRD](PRD.md), IDs RQ-001 a RQ-018.

As versões abaixo organizam entregas e dependências; não são datas ou promessas
de prazo. A formalização do projeto preserva o comportamento da 0.2. Cada marco
futuro precisa de contrato, implementação e aceitação próprios antes de ser
declarado concluído. A sequência retoma os
[marcos da arquitetura](../ARQUITETURA.md).

## 1. Decisões estabelecidas e escolhas abertas

### Decisões que orientam o trabalho

| Decisão | Consequência |
|---|---|
| Implementação própria em Rust e arquitetura de microkernel | RQ-001 a RQ-003 valem em todos os marcos; protocolos, filesystem e política de recuperação continuam em serviços. |
| Produto integrado | A entrega mantém imagem, versão e contratos coerentes; componentes internos podem evoluir em binários separados. |
| Plataforma virtual restrita como referência inicial | A linha de base é uma CPU x86-64, QEMU/TCG, OVMF e e1000 emulada; mudanças de plataforma devem declarar o que foi novamente validado. |
| 0.2 como base de regressão e 0.3 como próximo marco | Avançar primeiro a espera por eventos; memória e aplicações gerais não entram implicitamente no escopo da 0.3. |
| DMA sem IOMMU exige driver confiável | Arenas antigas da rede não passam a ser reutilizáveis por existir um alocador novo. A dependência de ciclo de vida DMA permanece. |

As decisões de arquitetura já adotadas estão nos
[ADR 001](../ADR-001-MICROKERNEL.md) e
[ADR 002](../ADR-002-CICLO-DE-VIDA.md).

### Hipóteses de planejamento e decisões a fechar

| Tema | Hipótese ou questão aberta | Resolver antes de |
|---|---|---|
| Espera e ABI | ADR 003 decide espera multiplexada, envio não bloqueante, limites e incompatibilidade sem tradução implícita; envio bloqueante fica fora de 0.3. Números e layout binário permanecem a definir antes da exposição. | Alterar o contrato da 0.3 |
| Temporização | Usar tiques monotônicos e implementar as regras de tentativa imediata, prazo finito, rollover e precedência decididas na ADR 003. Não assumir garantia de tempo real. | Implementar timeout |
| Primeira IRQ delegada | Piloto proposto: UART 16550 COM1 com IRQ4 exclusiva do terminal. Definir máscara, confirmação e coalescência; notificações do kernel ficam separadas da fila IPC. Rede e PS/2 permanecem em polling com prazo documentado. | Integrar notificações da 0.3 |
| Aplicações e armazenamento | Definir origem dos executáveis, política de carga, dispositivo de bloco, formato de disco e garantias de persistência. A ESP do boot não é um filesystem do SO em execução. | Abrir implementação das respectivas trilhas |
| IOMMU e suporte | Escolher plataforma de teste, dispositivo e comportamento esperado de revogação/reset. Antecipar essa trilha se o modelo passar a admitir driver DMA não confiável. | Ampliar o modelo de confiança ou reciclar buffers DMA |

Cada escolha deve registrar alternativas, motivo, contrato afetado e teste de
aceitação em ADR. Hipóteses não autorizam afirmar suporte ou garantias ainda não
demonstrados.

## 2. Linha de base 0.2

A [validação registrada](../../evidencias/RESUMO-VALIDACAO.md) reúne 26 testes
no host e cinco boots: integração/rede, memória, preempção, recuperação da rede
e recuperação de laço. Ela fixa o comportamento de partida, incluindo geração
de referências, autoridade do supervisor e quatro encarnações por boot.

Para uma nova entrega, preservar os casos relevantes com o artefato reconstruído,
registrar mudanças deliberadas e acrescentar cenários para os novos mecanismos.
O total de testes pode mudar; a cobertura dos requisitos e os resultados são o
critério de continuidade. Não converter o resultado antigo em evidência de uma
imagem nova.

## 3. Próximo marco detalhado: 0.3

**Resultado esperado:** um serviço pode esperar por trabalho e prazo sem manter
um laço de consultas; o núcleo acorda o destinatário correto e conserva controle
de recursos e preempção. Requisitos principais: RQ-009, RQ-010 e RQ-011.
A semântica foi decidida na [ADR 003](../ADR-003-ESPERA-0.3.md); o desenho está na [especificação da 0.3](ESPECIFICACAO-0.3.md), com
fronteiras no [modelo de ameaças](AMEACAS-E-LIMITES.md).

| Pacote | Entrega | Dependência | Aceitação |
|---|---|---|---|
| 03-A — contrato | ADR de espera, retorno de erro, prazo, cancelamento, IRQ e limites; separar estado de vida e estado de escalonamento; definição da compatibilidade; codificação binária antes da exposição. | Leitura da ABI 2 e dos casos de reinício/revogação. | Contrato cobre evento já disponível, destino falho/finalizado, referência antiga, fila cheia e empate evento/timeout; supervisor não confunde bloqueio com morte. |
| 03-B — bloqueio e despertar | Recepção bloqueante, transição atômica entre fila e espera, integração ao escalonador e estado ocioso do núcleo. | 03-A. | Mensagem chegando em cada lado da transição não se perde; bloqueado não roda; todos bloqueados permitem espera ociosa e retomada por evento. |
| 03-C — prazo e invalidação | Timeout, cancelamento e tratamento da morte/substituição da instância enquanto há espera. | 03-A e 03-B. | Cada espera termina uma vez; cliente não fica preso após revogação; evento atrasado não alcança a geração nova como se fosse da antiga. |
| 03-D — IRQ delegada | Piloto COM1/IRQ4 exclusiva para o terminal, com máscara/confirmação e notificações separadas da fila IPC. | 03-A e 03-B; contrato de eventos do dispositivo. | Terminal acorda por IRQ real; acesso sem direito falha; fila IPC cheia não oculta IRQ; repetição de IRQ e consumidor parado respeitam limites e preservam o timer de preempção. |
| 03-E — uso e regressões | Integrar terminal/supervisor à espera; usar prazos explícitos no polling restante de rede e PS/2; atualizar wrappers/docs e produzir imagem e relatórios. | 03-B, 03-C e 03-D. | Demonstrar serviço ocioso bloqueado, trabalho retomado e comportamento sob timeout/falha; repetir os cenários da base afetados e registrar os caminhos que ainda usam polling. |

Filas, esperas registradas e notificações precisam de limites explícitos. A fila
atual de 16 mensagens é a referência; qualquer alteração deve dizer como o
remetente ou receptor observa esgotamento. A etapa não deve transformar uma fila
cheia ou uma tempestade de eventos em alocação ilimitada.

**Saída da 0.3:** contratos e invariantes revisados, casos de corrida e falha
exercitados, testes do host separados dos de VM, documentação do novo artefato e
regressões aceitas. Redução de polling é demonstrada pelo estado/agendamento dos
serviços; promessas de economia percentual de CPU dependem de medição própria.

## 4. Trilhas seguintes e dependências

| Marco/trilha | Entrega e requisitos | Dependências explícitas | Critério de conclusão |
|---|---|---|---|
| 0.4 — memória e autoridade | Alocação/devolução de memória, objetos com direitos e revogação; RQ-012. | 0.3 para espera, cancelamento e término; ADR de propriedade, quotas e identidade de objetos. | Recursos elegíveis são recuperados após saída/falha; referências antigas e direitos insuficientes são recusados; memória DMA em uso/quarentena não entra no alocador livre. |
| Aplicações — trilha após 0.4 | Carregamento e ciclo de vida de programa independente; RQ-013. | 0.4, ABI de aplicações e política de carregamento. Pode iniciar com fonte controlada em memória; carga persistente depende da 0.5. | Terminal inicia/encerra programa sem recompilar kernel; imagem inválida é recusada e falha do programa preserva serviços. |
| 0.5 — armazenamento | Driver de bloco em usuário, serviço de arquivos e API; RQ-014. | 0.3, 0.4, dispositivo/formato escolhidos e política DMA compatível com o modelo de confiança. | Bytes confirmados sobrevivem ao reboot; corrupção e escrita interrompida têm resultado documentado e testado. |
| 0.6 — rede para aplicações | Separação de driver/pilha, buffers autorizados, API de rede, TCP, DHCP e DNS; RQ-015. | 0.3 para eventos/timeouts; 0.4 para propriedade e compartilhamento; trilha de aplicações para aceitar o uso por programa independente. | Aplicação comunica por API; perdas, retransmissão e erros são ensaiados; driver e protocolos permanecem fora do kernel. |
| 0.7 — IOMMU e DMA | Domínios de DMA e ciclo de vida de dispositivos/buffers; RQ-016. | 0.4, plataforma IOMMU selecionada e especificação de reset/revogação. Pode anteceder 0.5/0.6 se houver exigência de driver não confiável. | Acesso DMA fora do domínio falha; revogação, operações pendentes e reutilização de buffers são verificados no cenário definido. |
| Recuperação geral — trilha transversal | Dependências entre serviços, erros para clientes e recuperação dos componentes cobertos; RQ-017. | 0.3 e 0.4; contratos de persistência da 0.5 e de sessões da 0.6 quando aplicáveis; 0.7 para recuperação com reuso seguro de recursos DMA. | Falhas não deixam clientes esperando indefinidamente nem referências válidas para instâncias substituídas; política para falha do supervisor está definida e ensaiada. |
| Produto no escopo declarado | Aplicações, instalação, atualização e matriz de suporte; RQ-018. | Trilhas de aplicações, arquivos, rede e recuperação aceitas; política DMA demonstrada. IOMMU é obrigatória para admitir drivers DMA não confiáveis. | Boot/instalação e atualização reproduzíveis por procedimento; atualização interrompida recuperável; testes correspondem à plataforma publicada. |

O número do marco não obriga todas as trilhas a uma execução linear. Aplicações
podem evoluir em paralelo a armazenamento depois da memória dinâmica; IOMMU deve
ser antecipada quando for pré-requisito de confiança. Essa flexibilidade não
permite declarar uma entrega concluída antes de suas dependências de aceitação.

## 5. Promoção e manutenção do planejamento

As promoções de estágio seguem o [PRD, seção 5](PRD.md). Concluir a 0.3 melhora a
base de pesquisa; não produz, por si só, aplicações dinâmicas ou persistência.
“SO mínimo utilizável” exige o conjunto de cenários de produto definido na
[avaliação existente](../CRITERIOS-TANENBAUM.md), dentro da plataforma declarada.

Para abrir uma tarefa, indicar requisito, pacote/marco, dependências, contrato
afetado e evidência esperada. Para concluir, vincular a alteração ao resultado
observado e aos limites que continuam valendo. Decisões novas atualizam PRD,
roadmap, ADR e ABI quando correspondentes; os relatórios antigos preservam o
que foi realmente executado.

Datas, responsáveis individuais, capacidade da equipe, métricas de desempenho e
plataformas adicionais devem ser definidos quando houver informação para isso.
Nenhum deles é presumido por este roadmap.
