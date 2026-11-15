# Raiz OS — microkernel próprio em Rust

**Nome provisório. Versão experimental 0.2.0.** Projeto criado para Raphael Maquiné Marinho.

O objetivo é um sistema operacional próprio: carregador, microkernel, serviços,
drivers e protocolos escritos neste projeto, distribuídos como um produto único.
Não usa kernel Linux, BSD, Redox, seL4, libc ou uma distribuição como base.
As referências técnicas são especificações de firmware, CPU, formatos de arquivo
e protocolos; não são implementações incorporadas de outros sistemas.

## Continuar o projeto

Comece por [PROJETO.md](PROJETO.md): visão, base implementada, requisitos,
roadmap, backlog e próximo marco. O [guia de contribuição](CONTRIBUINDO.md)
explica como preparar o ambiente e trabalhar no código. As instruções para
agentes estão em [AGENTS.md](AGENTS.md).

A versão 0.3 tem [especificação proposta](docs/projeto/ESPECIFICACAO-0.3.md).
O código e a imagem deste pacote permanecem na versão 0.2.0, ABI 2.

## O que esta entrega contém

- Carregador UEFI em Rust com kernel ELF incorporado e `ExitBootServices`.
- Microkernel x86-64 com páginas por processo, GDT/IDT/TSS, transições ring 0/ring 3,
  escalonamento, chamadas de sistema e mensagens entre serviços.
- Processo `servico-terminal`: console serial, framebuffer, teclado PS/2 e comandos.
- Processo `servico-rede`: driver e1000, Ethernet, ARP, IPv4, ICMP e eco UDP.
- Processo `servico-supervisor`: parada e recuperação da rede, por comando ou
  por política automática limitada, fora do kernel.
- Referências de instância com geração; rejeição de referências antigas e
  remoção de mensagens pendentes da instância revogada.
- Permissões estáticas para IPC, portas e dispositivo PCI de cada serviço.
- Ferramenta de construção e teste em Rust, sem crates de terceiros.
- Imagem única `saida/raiz-os.img`, GPT + ESP FAT32, com 128 MiB.

O arquivo `evidencias/RELATORIO-SMOKE.md` registra o resultado real do teste do
pacote, quando presente. Código compilado, teste unitário e boot em QEMU são
evidências diferentes; a documentação não deve tratá-las como equivalentes.

## Avaliação pelos conceitos de Tanenbaum

A classificação atual é **sistema operacional experimental mínimo, de propósito
restrito, com microkernel próprio**. Há abstrações para programas e administração
de CPU, memória e dispositivos, com critérios de preempção, proteção de memória,
IPC, rede e recuperação. O produto completo pretendido ainda requer aplicações
dinâmicas, persistência, recuperação geral e ampliação dos serviços.

A matriz de avaliação, as fontes e os critérios verificáveis para os próximos
marcos estão em [Avaliação pelos conceitos de Tanenbaum](docs/CRITERIOS-TANENBAUM.md). A matriz é
nossa aplicação dos conceitos do livro, não uma certificação atribuída ao autor.

## Plataforma de referência

| Item | Seleção |
|---|---|
| Arquitetura | x86-64 |
| Firmware | UEFI, OVMF no teste |
| Máquina virtual | QEMU `pc` / i440fx, TCG |
| CPUs e RAM | 1 CPU, 256 MiB |
| Placa de rede | Intel 82540EM emulada, PCI 8086:100e |
| Console | UART 16550; framebuffer GOP RGB/BGR; PS/2 set 1 |
| Rede inicial | IPv4 10.0.2.15/24, gateway 10.0.2.2 |
| UDP | Eco em 7777, encaminhado para 127.0.0.1:17777 |

O suporte se limita a essa configuração. A imagem não é um instalador para
computadores físicos. Não contém o firmware OVMF nem o emulador QEMU.

## Compilar e executar

Pré-requisitos no computador de desenvolvimento: Rust via rustup, QEMU x86-64 e
OVMF. A versão do compilador e os alvos estão em `rust-toolchain.toml`.
O host de desenvolvimento não faz parte do sistema operacional produzido.

Na raiz do projeto:

```sh
cargo raiz compilar
cargo raiz verificar-imagem
cargo raiz executar
```

Se o firmware não for encontrado automaticamente, aponte para um par CODE/VARS
correspondente da instalação de OVMF:

```sh
export RAIZ_OVMF=/caminho/OVMF_CODE_4M.fd
export RAIZ_OVMF_VARS=/caminho/OVMF_VARS_4M.fd
cargo raiz executar
```

Para janela gráfica, use `cargo raiz executar --grafico`. O console serial
continua ativo. A fonte do framebuffer é uma fonte simples própria; letras
minúsculas e acentos são representados visualmente por formas maiúsculas básicas.
O teclado PS/2 usa layout US; a serial é o caminho principal de teste.

No PowerShell, as mesmas variáveis podem ser definidas com `$env:RAIZ_OVMF`,
`$env:RAIZ_OVMF_VARS` e `$env:RAIZ_QEMU`. Os caminhos dependem da instalação
local. O fluxo foi validado no host descrito nos relatórios; não foi validado
em Windows ou em máquina física nesta entrega.

**Atenção ao desenvolvimento:** `executar` e `testar` usam uma imagem existente.
Depois de editar código, execute `cargo raiz compilar` novamente.

## Comandos do terminal

| Comando | Função |
|---|---|
| `ajuda` | Lista comandos implementados |
| `sobre` | Identifica microkernel, serviços e CPL do terminal |
| `memoria` | Mostra reservas e organização dos espaços virtuais |
| `processos` | Estados dos serviços e contador de timer |
| `rede` | Solicita estatísticas ao serviço de rede por IPC |
| `ping 10.0.2.2` | Solicita um ICMP Echo ao gateway virtual |
| `eco texto` | Escreve texto no console |
| `limpar` | Limpa tela e serial |
| `teste-cap` | Confirma que o terminal não pode consultar PCI |
| `falhar-rede` | Provoca instrução inválida no processo de rede |
| `violar-memoria` | Faz a rede tentar ler uma página do núcleo |
| `ocupar-rede` | Coloca a rede em laço sem ceder, para testar preempção |
| `supervisao` | Consulta modo, reinícios e limite do supervisor |
| `supervisao auto` | Habilita recuperação após falha ou término da rede |
| `supervisao manual` | Desabilita recuperação automática |
| `reiniciar-rede` | Solicita ao supervisor uma nova instância da rede |
| `parar-rede` | Muda para modo manual e encerra a instância da rede |
| `guardar-referencia` | Guarda a referência atual para um teste posterior |
| `teste-referencia` | Exige recusa da referência guardada após um reinício |
| `teste-supervisor` | Confirma que terminal não pode reiniciar/encerrar diretamente |

`falhar-rede`, `violar-memoria` e `ocupar-rede` são ensaios deliberados do
laboratório. O modo inicial de supervisão é manual: depois deles, use
`reiniciar-rede` para recuperar a rede. O modo automático observa falha/término;
não detecta sozinho um serviço preso em laço.

**Limite da versão:** quatro encarnações da rede por boot, contando a inicial
(três reinícios). Cada uma usa uma arena física nova de 2 MiB; as antigas ficam
em quarentena. Esgotar o orçamento recusa o reinício e preserva uma rede que
ainda esteja funcionando. Um reboot renova o orçamento. Essa política não
substitui IOMMU nem permite confiar em um driver DMA hostil.

## Verificação automatizada

```sh
cargo test -p protocolos -p ferramentas -p contrato-boot -p contrato-sistema
cargo test -p nucleo --lib
cargo raiz compilar
cargo raiz testar
cargo raiz testar-isolamento
cargo raiz testar-recuperacao
```

O roteiro inicia sua própria VM, aguarda os serviços, solicita ping, verifica
eco UDP, testa uma permissão negada e provoca a morte do processo de rede.
Depois exige uma nova resposta do terminal e a indicação de serviço indisponível
em uma nova operação IPC. Os resultados ficam em `evidencias/`.

`testar-isolamento` inicia duas VMs em sequência: uma para verificar a recusa de
acesso à memória supervisor e outra para verificar preempção enquanto a rede
fica em um laço sem chamadas ao kernel. Cada cenário conserva seu próprio log.

`testar-recuperacao` verifica o supervisor em CPL 3, negação de autoridade ao
terminal, reinício manual após falha, referências antigas, recuperação automática,
parada/retomada e quota esgotada sem derrubar uma rede viva. Em outro boot, verifica
recuperação de uma rede em laço. Ping e UDP são exigidos após a recuperação.
Os relatórios conservam o resultado real de cada cenário.

Com uma VM aberta em outro terminal:

```sh
cargo raiz eco-udp "ola do host"
```

Esse cliente fala apenas com `127.0.0.1:17777`. O teste usa a rede virtual do
QEMU e demonstra a nossa pilha no convidado; não é um teste de uma NIC física,
da Internet pública ou de desempenho de produção.

## Como o produto único é construído

1. Compilar três serviços ELF independentes para bare metal.
2. Incorporar esses ELFs à imagem do microkernel de bootstrap.
3. Incorporar o ELF do microkernel ao carregador UEFI.
4. Colocar `BOOTX64.EFI` na partição ESP da imagem de disco.

O usuário recebe um único `raiz-os.img`. Na execução, o microkernel cria espaços
de endereço separados para os serviços. Incorporar binários na distribuição
não significa executar todos no mesmo domínio de privilégio.

## Organização do código

| Diretório | Responsabilidade |
|---|---|
| `inicializador` | UEFI, validação ELF, reserva de memória e passagem ao núcleo |
| `contrato-boot` | ABI carregador/núcleo |
| `nucleo` | CPU, memória, contextos, agendamento, IPC e permissões |
| `contrato-sistema` | ABI de syscalls, mensagens e informação dos serviços |
| `apoio-servicos` | Pequenos wrappers Rust para `int 0x80` |
| `servicos/terminal` | Interface textual e drivers do console |
| `servicos/rede` | Driver e1000 e serviço de rede |
| `servicos/supervisor` | Política de parada, recuperação e retentativas |
| `protocolos` | Parsing/checksums independentes do hardware, testáveis no host |
| `ferramentas` | Construtor GPT/FAT32, emulador e testes de integração |
| `docs` | Arquitetura, decisões, ABI, limites e próximos marcos |
| `evidencias` | Logs e resultados observados |

## Limites atuais

É uma fundação executável de pesquisa, ainda distante de um SO de uso geral.
Não há filesystem no kernel ou em serviço, armazenamento após boot, processos
dinâmicos de propósito geral, TCP, DHCP, DNS, IPv6, TLS, SMP, USB, Wi-Fi,
interfaces gráficas de aplicações ou instalação em hardware real.

O reinício é restrito à rede e ao orçamento descrito; não restaura seu estado de
protocolos. Terminal e supervisor ainda não são recuperáveis. O supervisor
consulta estados e cede CPU; IPC bloqueante e notificações de morte ficam para a
próxima etapa. O boot e a reserva de memória permanecem estáticos.

As permissões são fixas por serviço; não há tabela dinâmica de capabilities com
delegação e revogação. Não há IOMMU: o driver DMA precisa ser confiável. Uma
falha de CPU contida não prova isolamento contra DMA malicioso ou todos os erros
possíveis de hardware. Veja `docs/ARQUITETURA.md` e `docs/ADR-001-MICROKERNEL.md`.

O código do SO está em Rust, com `unsafe`, pequenos blocos de assembly embutido
e scripts de ligação. Firmware, microcódigo, compilador e emulador são externos
ao produto. “Escrito em Rust” não significa “100% safe Rust” ou correção provada.

## Documentação da evolução

- [Decisão de microkernel](docs/ADR-001-MICROKERNEL.md).
- [Supervisor, referências e recuperação](docs/ADR-002-CICLO-DE-VIDA.md).
- [ABI de serviços 0.2](docs/ABI-0.2.md), incompatível com os binários ABI 1.
- [Arquitetura atual](docs/ARQUITETURA.md).
- [Critérios e avaliação por Tanenbaum](docs/CRITERIOS-TANENBAUM.md).
- [Resultado da validação](evidencias/RESUMO-VALIDACAO.md).

As evidências de 0.1 foram conservadas em `evidencias/v0.1`. Os resultados na raiz
de `evidencias/` correspondem à execução corrente descrita no resumo.

