# Critérios de sistema operacional e avaliação do Raiz OS

**Projeto:** Raiz OS 0.2.0, nome provisório.  
**Revisão desta avaliação:** 9 de novembro de 2026, America/Boa_Vista (UTC−04).  
**Decisões do usuário:** implementação própria em Rust; microkernel; produto integrado; usar os conceitos de Andrew S. Tanenbaum para avaliar a classificação.

## 1. Conclusão e alcance

**A classificação sustentada pela implementação e pelos testes é: sistema
operacional experimental mínimo, de propósito restrito, com microkernel próprio.**

Essa é uma avaliação de engenharia do projeto. Não é uma certificação de
Tanenbaum nem a afirmação de que o produto completo solicitado já está pronto.
Os programas que o sistema administra atualmente são três serviços incorporados
à imagem: terminal, rede e supervisor. Ainda não existe um ambiente geral de aplicações.

O fundamento da classificação é a combinação de interfaces para programas com
administração efetiva de CPU, memória e dispositivos. O boot e a aparência do
terminal, isoladamente, não são o teste de aceitação adotado aqui.

## 2. O que estamos tomando de Tanenbaum

Tanenbaum e Bos apresentam o SO sob duas perspectivas: oferecer abstrações úteis
aos programas e administrar os recursos de hardware. Distinguem também a
interface de usuário dos serviços que ela utiliza. O livro discute diversas
organizações e tipos de SO; as seções consultadas não estabelecem uma lista
normativa universal de certificação. [1, seções 1.1.1, 1.1.2 e 1.5.6]

O artigo de Tanenbaum e coautores sobre MINIX 3 descreve a organização
multisservidor, com núcleo pequeno e componentes em processos de usuário, e um
serviço específico para recuperação. Usamos essa referência conceitual para
distinguir isolamento de recuperação automática. Não incorporamos sua
implementação nem assumimos suas garantias. [2]

**Daqui em diante, os critérios, resultados e marcos são nossos:** transformam
esses conceitos em condições verificáveis para este projeto. Eles não significam
que todo SO precise de GUI, POSIX, TCP, filesystem, preempção ou ring 3.

## 3. Matriz de avaliação da versão 0.2

| Critério deste projeto | O que existe | Evidência e limite | Avaliação |
|---|---|---|---|
| Assumir a execução da máquina | Carregador UEFI próprio, ELF validado e transferência ao núcleo após `ExitBootServices` | Log de boot; a execução posterior usa drivers do projeto. Firmware permanece uma dependência da plataforma. | Demonstrado na VM |
| Oferecer abstrações a programas | Processo, espaço virtual, estado de serviço, mensagem e chamadas de sistema | Terminal solicita rede por IPC; API limitada aos serviços atuais. | Presente em nível mínimo |
| Administrar CPU | Contextos independentes, timer e escalonamento preemptivo | Rede fica em laço sem ceder e o terminal continua respondendo. Uma CPU e três processos. | Demonstrado |
| Administrar memória | Reservas físicas separadas, tabelas de páginas por processo e permissões de acesso | Leitura de memória supervisor provoca page fault no serviço infrator. Reservas e população de processos são estáticas. | Demonstrado no caso ensaiado |
| Administrar entrada e saída | Portas, janelas MMIO e acesso PCI atribuídos por serviço | Terminal não pode usar a operação PCI concedida à rede. Suporte restrito ao hardware declarado. | Presente e testado |
| Mediar comunicação | Syscalls `int 0x80`, identidade do remetente definida pelo kernel e filas limitadas | Comunicação terminal/rede; envio a serviço morto retorna erro. Sem IPC bloqueante ou delegação dinâmica. | Presente em nível mínimo |
| Separar autoridade | Ring 0 para mecanismos do núcleo; terminal, rede e supervisor em ring 3 | Serviços informam CPL 3; terminal não pode reiniciar diretamente a rede. Sem IOMMU ou capabilities dinâmicas. | Parcial, com limites explícitos |
| Conter falhas de CPU de um serviço | Exceção em usuário marca o serviço como falho e permite executar os demais serviços executáveis | Instrução inválida e leitura proibida da rede preservam o terminal. Falhas do próprio kernel continuam fatais. | Demonstrado nos dois casos |
| Oferecer persistência | A imagem tem ESP FAT32 para boot, mas o SO não a usa como filesystem em execução | Não há driver de bloco ou serviço de arquivos após boot. | Ausente |
| Executar aplicações independentes | O bootstrap carrega três ELFs incorporados | Não há criação dinâmica, carregamento pelo terminal ou execução de um quarto programa sem reconstruir a imagem. | Ausente como serviço geral |
| Recuperar serviços | Supervisor em ring 3 administra reinício da rede, com geração e descarte de mensagens antigas | Recuperação manual/automática, referência antiga e quota verificadas; restrita à rede, sem restauração de estado. | Demonstrado no escopo limitado |
| Oferecer rede geral | Ethernet, ARP, IPv4, ICMP e eco UDP | Ping e 84 datagramas verificados; sem TCP, DHCP, DNS, IPv6, TLS ou API geral de sockets. | Parcial |

A ausência de persistência ou de rede completa delimita nosso produto pretendido;
não é usada como uma suposta regra universal para negar a existência de qualquer
SO de propósito restrito. A plataforma já exerce funções de SO, mas seu alcance
deve acompanhar essa afirmação em toda apresentação da versão.

## 4. Evidência executada

### 4.1 Ambiente

- x86-64, QEMU/TCG, máquina `pc` / i440fx, uma CPU e 256 MiB de RAM.
- Firmware OVMF; dispositivo Intel 82540EM/e1000 emulado.
- Imagem GPT com ESP FAT32 construída pelas ferramentas Rust do projeto.
- IPv4 fixo `10.0.2.15`, gateway virtual `10.0.2.2`, eco UDP em `7777`.

Esses resultados não são validação em uma placa de rede física ou em um computador
de uso cotidiano. O emulador e o firmware não estão incluídos na distribuição.

### 4.2 Boot, serviços, rede e permissões

O teste `cargo raiz testar` verificou a estrutura da imagem, observou o fim dos
serviços de boot, confirmou CPL 3 de terminal e rede e recebeu resposta ICMP do
gateway virtual. Comparou byte a byte 84 respostas UDP, totalizando 6.610 bytes de
payload, incluindo datagramas com 0, 1, 17 e 1.472 bytes e a volta dos anéis DMA.

O teste também exigiu a negação da operação PCI ao terminal. Depois provocou
instrução inválida no serviço de rede, confirmou a continuidade do terminal e
exigiu erro de serviço indisponível numa nova solicitação IPC.

Evidências: [relatório de integração](../evidencias/RELATORIO-SMOKE.md) e
[log serial](../evidencias/serial-smoke.log).

### 4.3 Memória protegida

O teste `cargo raiz testar-isolamento` iniciou uma nova VM e fez o serviço de rede
tentar ler `0x02000000`, endereço reservado ao núcleo. O log registrou page fault,
vetor 14, erro `0x5`, CR2 igual ao endereço tentado e falha apenas do serviço 2.
Depois o terminal respondeu ao comando de eco exigido pelo teste.

Isso demonstra a aplicação da proteção de páginas naquele acesso de CPU. Não
testa todos os endereços, escrita, execução indevida ou acesso de dispositivo DMA.

Evidência: [log de memória](../evidencias/serial-isolamento-memoria.log).

### 4.4 Preempção

Em outra inicialização, a rede entrou deliberadamente em um loop que não faz
chamadas de cessão ao kernel. Após a notificação `LOOP_SEM_CEDER`, o terminal
respondeu com a linha exata `PREEMPCAO_OK`. Consultas posteriores mostraram terminal
e rede executáveis e avanço do contador de timer de 53 para 83 na execução 0.2.

Isso evidencia que a CPU foi retomada e entregue ao terminal sem cooperação da
rede. Não é uma medida de latência máxima, um benchmark ou garantia de tempo real.

Evidências: [log de preempção](../evidencias/serial-isolamento-preempcao.log) e
[relatório dos dois cenários](../evidencias/RELATORIO-ISOLAMENTO.md).

### 4.5 Verificações no host

Passaram 26 testes: nove para protocolos, cinco para ferramentas, três para a ABI
e nove para mecanismos puros do núcleo. Além dos casos de pacotes e imagem, há
testes de codificação/layout, filas circulares, purga por origem, autoridade,
referências antigas, quota e análise da cadeia de arenas registrada no boot.
Os testes no host complementam as cinco inicializações verificadas; não substituem
a execução bare metal do convidado. O log está em
[testes no host](../evidencias/testes-host-0.2.log).

### 4.6 Recuperação acrescentada na versão 0.2

O teste `cargo raiz testar-recuperacao` passou em duas VMs independentes. Na
primeira, uma falha por instrução inválida foi seguida de reinício manual da rede;
uma leitura proibida em modo automático foi seguida de outra recuperação; e uma
parada intencional foi seguida de retomada. As gerações observadas foram 1, 2, 3 e
4, com uma única ocorrência de `EXIT_BOOT_SERVICES_OK` no cenário.

A referência conservada da geração 1 foi recusada para envio e consulta de estado.
O terminal também recebeu recusa ao tentar usar diretamente os mecanismos de
reinício e encerramento reservados ao supervisor. Ao solicitar uma quinta
encarnação, o kernel retornou falta de recursos e a geração 4 continuou funcional.

Na segunda VM, uma rede em loop sem ceder foi substituída por comando do usuário.
O teste exigiu resposta do terminal antes da recuperação e retorno do tráfego
depois. Ao todo, os dois cenários compararam 24 respostas UDP, com 8.940 bytes de
payload; ping e UDP foram verificados após cada recuperação e após a recusa por
quota. As arenas físicas distintas foram conferidas no log.

Evidência: [relatório de recuperação](../evidencias/RELATORIO-RECUPERACAO.md).
A purga seletiva em filas ocupadas foi exercitada pelos testes do módulo real
de controle do kernel; nos reinícios dos cenários de VM, as caixas estavam vazias
no momento da troca. Não se atribui ao ensaio de VM uma cobertura que ele não teve.

## 5. O compromisso de usar microkernel

O microkernel atual mantém contextos de CPU, paginação, preempção, entrada de
interrupções/exceções, IPC e aplicação de permissões. O terminal, os drivers de
console, o driver e1000 e os protocolos de rede executam fora do ring 0.

Na versão 0.2 o supervisor também executa em ring 3. Ele escolhe quando reiniciar;
o kernel aplica direitos, prepara o novo contexto e publica a referência de nova
geração. Recuperação é uma capacidade adicional à contenção já ensaiada.

O produto pode continuar sendo uma imagem única. O construtor incorpora os ELFs
dos serviços à distribuição e o núcleo os carrega em espaços separados. Uma
imagem integrada não exige um único domínio de privilégio.

Essa escolha implica contratos de comunicação explícitos, estado de serviço,
erros, filas e tratamento de falhas parciais. Adicionar TCP ou filesystem ao
projeto deverá produzir serviços, preservando a decisão arquitetural.

O bootstrap ainda faz escolhas estáticas de processos, memória e PCI. Elas estão
registradas como concessões temporárias, não como um desenho acabado de separação
entre mecanismo e política. Driver e protocolos de rede também compartilham o
mesmo processo nesta versão.

### Isolamento e recuperação são entregas distintas

Os testes de isolamento demonstram contenção das falhas de CPU provocadas. A etapa
0.2 acrescenta recriação da rede, referência de geração, descarte de mensagens
pendentes e reinicialização do dispositivo. O serviço começa com estado vazio;
operações antigas não são retomadas. A recuperação geral de dependências e de
outros serviços continua pendente.

### A proteção de DMA continua pendente

A NIC usa DMA. Sem IOMMU, limitar os acessos de CPU do driver não limita os
endereços físicos que o dispositivo pode acessar sob seu comando. [3] Por isso o
driver de rede deste projeto ainda precisa ser confiável. Na implementação atual,
desativar bus mastering após uma falha, conservar os buffers e usar uma nova arena
para a encarnação seguinte não prova que todas as operações pendentes cessaram.
Essa fronteira precisa de implementação e testes próprios antes de reivindicar
confinamento de drivers hostis.

### Rust e desempenho

Os componentes do SO usam Rust `no_std`, com `unsafe` e assembly embutido nas
fronteiras de máquina. A linguagem não substitui os contratos de autoridade,
paginação, DMA e sincronização. Não há prova formal de correção nesta entrega.

O caminho de IPC e troca de contexto tem trabalho mensurável. Antes de prometer
desempenho, mediremos latências, utilização de CPU e tráfego sob carga. O teste de
eco atual verifica resultado funcional. As mensagens transportam poucos valores;
pacotes permanecem no serviço de rede. Memória compartilhada autorizada e
notificações bloqueantes são trabalho futuro.

## 6. Critérios de aceitação para a evolução do produto

Estes são requisitos de engenharia que adotamos para o objetivo do usuário,
não uma tabela atribuída ao autor do livro. O próximo rótulo, **SO mínimo
utilizável**, exigirá passar pelos cenários abaixo e declarar o hardware suportado.

| Entrega | Teste de aceitação proposto |
|---|---|
| Ciclo de vida de aplicações | O terminal inicia e encerra um programa independente dos três serviços incorporados, sem recompilar o kernel; recursos voltam ao gerenciador após o encerramento. |
| Memória dinâmica e autoridade | Processos obtêm e devolvem memória dentro de limites; operações sem direitos e identificadores revogados são recusados. |
| Espera por eventos | Serviço ocioso dorme; interrupção ou mensagem o acorda; cliente parado não causa crescimento ilimitado de filas. |
| Persistência em serviço de usuário | Criar, ler e alterar arquivo; reiniciar a máquina e verificar os bytes; testar falhas e corrupção segundo o formato escolhido. |
| Recuperação geral de serviços | Ampliar o mecanismo limitado da rede para os demais componentes, tratando dependências, referências antigas e recuperação de recursos. |
| Rede utilizável por aplicações | Uma aplicação abre comunicação por API documentada; testar TCP, perdas, retransmissão, DHCP e DNS conforme o escopo de rede escolhido. |
| Dispositivos e DMA | Definir o modelo de confiança; para drivers não confiáveis, confinar DMA e verificar que buffers revogados não são reutilizados enquanto acessíveis ao dispositivo. |
| Entrega integrada | Instalação/boot e atualização reproduzíveis, com recuperação de atualização interrompida e matriz explícita de hardware. |

Uma GUI, Wi-Fi e suporte amplo a periféricos são ampliações do produto; não serão
usados para substituir os testes fundamentais de abstração, administração de
recursos e isolamento.

## 7. Descrição da versão

> O Raiz OS 0.2 é um sistema operacional experimental mínimo com microkernel
> próprio em Rust. Ele administra três serviços isolados, CPU, memória e recursos
> de dispositivos, oferecendo chamadas de sistema e comunicação por mensagens.
> Sua execução e os cenários documentados de rede, preempção e proteção de memória
> foram testados em QEMU. O ambiente geral de aplicações, persistência e
> recuperação geral de serviços ainda está em desenvolvimento. A rede possui
> recuperação limitada por supervisor, com referências de instância e quarentena.

## Referências

1. TANENBAUM, Andrew S.; BOS, Herbert. *Modern Operating Systems*. 4th ed., Global
   Edition. Pearson, 2015. Capítulo 1, especialmente seções 1.1.1, 1.1.2, 1.5.6 e
   1.7.3; páginas impressas 3–6, 45–46 e 65–68.
   [Prévia da editora](https://api.pageplace.de/preview/DT0400.9781292061955_A24588869/preview-9781292061955_A24588869.pdf).
2. TANENBAUM, Andrew S. et al. *MINIX 3: Status Report and Current Research*.
   USENIX ;login:, junho de 2010.
   [Artigo](https://www.usenix.org/system/files/login/articles/61781-tanenbaum.pdf).
3. Trustworthy Systems. *Device Drivers*. Discussão de isolamento, IOMMU e buffers
   compartilhados.
   [Página do grupo de pesquisa](https://trustworthy.systems/projects/drivers/).

As fontes fundamentam conceitos. Código, resultados, limitações e critérios de
aceitação desta avaliação pertencem ao projeto Raiz OS.
