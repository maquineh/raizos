# Validação e entrega do Raiz OS

Este documento define como selecionar verificações, conservar evidências e
preparar uma entrega. A base versionada é a versão experimental **0.2.0**, ABI 2.
O RZ-022 foi concluído no escopo registrado em 15 de novembro de 2026. O RZ-023
foi concluído no escopo do aceite original: 12 comandos opt-in após rebuild,
testes host e cinco boots normais, com registros no [resumo](../../evidencias/execucao-rz-023-20261115/RESUMO.md)
e na [matriz T03](../../evidencias/execucao-rz-023-20261115/MATRIZ.md). A matriz
mantém lacunas por cenário; nem todos os T03 estão demonstrados. O marco 0.3
continua aberto e a ABI vigente permanece 2.
O RZ-024 também foi concluído no recorte controlado de idle/despertar descrito no
[resumo da execução](../../evidencias/execucao-rz-024-20261115/RESUMO.md); essa
validação não fecha o marco 0.3 nem converte observações finitas em garantias de
desempenho ou suporte físico.

Os resultados das seções que descrevem a entrega 0.2 são **históricos dessa
validação**, cuja data de referência registrada é 9 de novembro de 2026,
America/Boa_Vista. As seções posteriores identificam execuções incrementais de
0.3 por seus próprios logs e resumos. A redação e a revisão destes documentos
não executaram novamente testes Rust, compilação do SO ou VMs. A integridade dos arquivos do pacote é conferida separadamente durante
o empacotamento; essa conferência não renova os resultados funcionais do binário.

## 1. Base verificada e suas fontes

O [resumo da validação de 0.2](../../evidencias/RESUMO-VALIDACAO.md) registra
compilação sem avisos, **26 testes no host aprovados** e **cinco boots de
integração aprovados**. O [log dos testes no host](../../evidencias/testes-host-0.2.log)
permite conferir a contagem sem incluir crates ou doc-tests com zero casos.

| Grupo histórico | Casos aprovados | Fonte |
| --- | ---: | --- |
| Parsing e checksums de protocolos | 9 | `evidencias/testes-host-0.2.log` |
| Ferramentas de imagem e verificação de logs | 5 | `evidencias/testes-host-0.2.log` |
| Contrato de serviços e layout da ABI 2 | 3 | `evidencias/testes-host-0.2.log` |
| Filas, identidade, autoridade e quotas do núcleo | 9 | `evidencias/testes-host-0.2.log` |
| Total | **26** | Soma dos quatro grupos acima |

`contrato-boot` participou do comando histórico, com zero testes próprios. Os
resultados de host não demonstram, por si sós, que uma imagem inicia.

| Comando de integração existente | Boots | Resultado histórico e limite |
| --- | ---: | --- |
| `cargo raiz testar` | 1 | Boot, CPL 3, ICMP, 84 ecos UDP, recusa de acesso PCI e terminal vivo após UD2 da rede. [Relatório](../../evidencias/RELATORIO-SMOKE.md). |
| `cargo raiz testar-isolamento` | 2 | Leitura proibida de `0x02000000` contida por page fault; em outro boot, terminal responde enquanto a rede fica em laço sem ceder. [Relatório](../../evidencias/RELATORIO-ISOLAMENTO.md). |
| `cargo raiz testar-recuperacao` | 2 | Supervisor, gerações, referências antigas, recuperação manual/automática, parada/retomada, quota e recuperação de laço. [Relatório](../../evidencias/RELATORIO-RECUPERACAO.md). |
| Total | **5** | Configuração virtual de referência; não são cinco execuções em hardware físico. |

Os dois cenários de recuperação compararam mais 24 respostas UDP. O conjunto
histórico totaliza **108 respostas e 15.550 bytes de payload conferidos**. No
ciclo principal, os logs mostram gerações 1 a 4 e arenas físicas distintas
`0x249a000`, `0x269a000`, `0x289a000` e `0x2a9a000`. Cada cenário de recuperação
contém um único `EXIT_BOOT_SERVICES_OK`, sem reboot durante as trocas.
Esses números pertencem à execução registrada, não são valores que toda futura
execução deva reproduzir. [Fonte](../../evidencias/RESUMO-VALIDACAO.md).

Nos reinícios das VMs, as caixas estavam vazias no momento da substituição.
A purga de filas ocupadas e circulares foi exercitada pelos testes do módulo
de controle usado pelo kernel. Não registrar esse caso como cobertura de VM.
[Fonte](../../evidencias/RESUMO-VALIDACAO.md).

## 2. Pré-requisitos e plataforma de referência

Execute os comandos de desenvolvimento na raiz do repositório. A configuração
está em [rust-toolchain.toml](../../rust-toolchain.toml),
[.cargo/config.toml](../../.cargo/config.toml) e no
[README](../../README.md).

| Requisito | Configuração |
| --- | --- |
| Rust e Cargo | Toolchain `1.99.1` via rustup; componentes `rustfmt` e `clippy` declarados no arquivo de toolchain |
| Alvos de compilação | `x86_64-unknown-none` e `x86_64-unknown-uefi` |
| Emulador | QEMU x86-64 com TCG; o histórico usou QEMU 10.0.13 |
| Firmware de teste | Par OVMF CODE/VARS correspondente; histórico com arquivos de 4 MiB |
| Máquina convidada | `pc`/i440fx, uma CPU, 256 MiB, e1000/82540EM emulada |
| Arquivos locais | Permissão para gravar `target/`, `saida/` e `evidencias/`; espaço para a imagem de 128 MiB e os artefatos de compilação |
| Rede de teste | Porta UDP local `127.0.0.1:17777` livre; encaminhamento para `10.0.2.15:7777` |

As versões de QEMU e OVMF efetivamente usadas em uma nova execução devem ser
registradas. Uma troca de firmware, máquina virtual ou versão de emulador é uma
mudança de ambiente que pode exigir nova integração. O histórico não comprova
execução em Windows ou em computador físico.

As ferramentas aceitam estas variáveis, quando o ambiente não for descoberto
automaticamente:

| Variável | Uso |
| --- | --- |
| `RAIZ_QEMU` | Caminho do executável QEMU ou wrapper local |
| `RAIZ_QEMU_DADOS` | Diretório de ROMs/dados, passado a QEMU com `-L` |
| `RAIZ_OVMF` | Arquivo OVMF CODE |
| `RAIZ_OVMF_VARS` | Arquivo OVMF VARS compatível com CODE |

Exemplo de configuração em um shell do host, ajustando os caminhos reais:

```sh
export RAIZ_OVMF=/caminho/OVMF_CODE_4M.fd
export RAIZ_OVMF_VARS=/caminho/OVMF_VARS_4M.fd
```

Firmware, emulador e compilador são dependências externas de desenvolvimento ou
plataforma. Não passam a fazer parte do código do SO ou da imagem por serem usados
na validação.

## 3. Comandos existentes e o que cada um demonstra

### Formatação e testes no host

```sh
cargo fmt --all --check
cargo test -p protocolos -p ferramentas -p contrato-boot -p contrato-sistema
cargo test -p nucleo --lib
```

O último comando seleciona a biblioteca de mecanismos testáveis do núcleo.
Os binários bare metal seguem seu fluxo próprio de construção. Não substituir
essa seleção por um suposto comando de teste de todo o workspace sem verificar
sua compatibilidade com os alvos e variáveis de construção.

Para uma mudança localizada, selecione o pacote afetado, por exemplo
`cargo test -p protocolos`. A matriz da seção seguinte define quando os ensaios
de VM também são necessários. Ter `clippy` instalado não significa que uma
execução de Clippy tenha sido registrada como gate de 0.2.

### Construção do produto

```sh
cargo raiz compilar
```

O construtor compila os três serviços, incorpora seus ELFs ao microkernel,
incorpora o kernel ao carregador UEFI e escreve a imagem GPT/FAT32. Ele também
reabre a imagem e verifica a estrutura e os bytes EFI gravados. Os resultados
principais são `saida/raiz-os.img` e `saida/esp/EFI/BOOT/BOOTX64.EFI`.

Para verificar uma imagem recebida ou já existente, sem reconstruir:

```sh
cargo raiz verificar-imagem
```

**Os comandos `executar`, `testar`, `testar-isolamento` e `testar-recuperacao`
utilizam uma imagem existente.** Depois de mudar código do SO, contratos,
linker scripts ou parâmetros que afetem os binários, execute `compilar` antes
de atribuir resultados ao código novo. Uma VM bem-sucedida com imagem antiga
não valida a alteração.

### Integração completa da base atual

```sh
cargo raiz testar
cargo raiz testar-isolamento
cargo raiz testar-recuperacao
```

Execute em sequência. Os comandos usam a mesma porta UDP e a mesma área de
saída; não executá-los simultaneamente. Cada cenário inicia e encerra sua VM,
inclusive quando uma verificação falha. Os relatórios são escritos em
`evidencias/`, com os nomes definidos pelas ferramentas.

Os ensaios de falha são parte intencional da aprovação: um vetor 6 ou 14 no
serviço da rede só conta como sucesso quando os demais resultados exigidos
também aparecem. Um log isolado de exceção não demonstra contenção ou recuperação.

### Inspeção interativa

```sh
cargo raiz executar
```

Com essa VM aberta, outro terminal pode executar:

```sh
cargo raiz eco-udp "ola do host"
```

`cargo raiz executar --grafico` habilita a janela de vídeo. Inspeção manual ou
uma captura de tela complementam o registro; não substituem a comparação dos
bytes e os resultados do roteiro automatizado. Encerre a VM interativa antes
dos comandos de integração que precisam da porta 17777.

## 4. Gate mínimo conforme a mudança

O autor da alteração deve indicar quais linhas desta matriz se aplicam e por
quê. Amplie a verificação quando o diff revelar uma dependência concreta;
não repetir todas as suítes apenas para aumentar a quantidade de checks.

| Tipo de mudança | Verificação mínima antes de considerar o trabalho concluído |
| --- | --- |
| Texto, links, backlog ou metadados sem efeito em build/runtime | Revisar diff, links locais, distinção entre fato/proposta e estrutura do CSV. Conferir IDs, dependências e critérios. Nenhum rebuild ou boot obrigatório. |
| Parser/checksum em `protocolos` | Testes do pacote afetado e casos de borda ligados à alteração. Se o código é usado na rede executável: recompilar e executar `testar`. Acrescentar recuperação apenas se estado ou reinicialização mudou. |
| Construtor GPT/FAT32 ou carregador | Testes de ferramentas/contrato afetados, recompilar e executar `testar`. Alteração de handoff, mapeamentos ou imagens de serviços exige também os dois comandos de isolamento/recuperação. |
| Harness de teste ou parser de log | Testes de `ferramentas` e execução do comando de integração cujo oráculo foi alterado. Revisar se eco de comando, saída parcial ou evento anterior poderiam gerar falso positivo. |
| Filas, validação de identidade ou direitos | `cargo test -p nucleo --lib`, testes de contrato afetados, recompilar e executar `testar` e `testar-recuperacao`. Mudança no caminho de escalonamento/bloqueio acrescenta `testar-isolamento`. |
| Contexto de CPU, paginação, interrupções, escalonamento ou ABI | Formatação, testes de host afetados, reconstrução de todos os binários pelo construtor e os três comandos de integração. Incluir caso específico para a nova regra ou fronteira. |
| Ciclo de vida, supervisor, arenas ou inicialização do e1000 | Testes do núcleo/contratos/ferramentas afetados, recompilar, `testar` e `testar-recuperacao`. Acrescentar `testar-isolamento` se interrupções, permissões de páginas ou preempção forem afetadas. |
| Toolchain, opções de compilação, firmware ou perfil de máquina de referência | Identificar versões efetivas, construir, executar o conjunto host e os cinco boots. Não transportar silenciosamente a aprovação de outro ambiente. |
| Implementação futura de 0.3 | Gate da área alterada mais os novos testes de espera, prazo, cancelamento, notificação e IRQ definidos na especificação. Os cinco boots de regressão continuam necessários no fechamento do marco. |

Esta é uma política proposta de revisão, baseada nos comandos existentes.
**Não há pipeline de CI criado ou validado por este documento.** Automatizar os
mesmos passos em um provedor exige configurar o ambiente, verificar a captura de
falhas/artefatos e executar o pipeline antes de chamá-lo de operacional.

## 5. Preservar evidências e associá-las aos binários

Os comandos atuais sobrescrevem os relatórios e logs de mesmo nome. Antes de
uma execução que substituirá uma referência útil, copie os arquivos relevantes
para uma pasta de execução identificada e ainda inexistente. Preserve os
originais completos, inclusive stderr e tentativas que falharam. Não limpar
trechos de falha para produzir um histórico com aparência de sucesso.

Uma execução que sustenta uma entrega deve reunir:

1. Identificação do código: commit quando houver Git, estado do diff e versão
   declarada. Para uma árvore sem commit, registre hashes dos arquivos efetivos.
2. Ambiente observado: versões do compilador, Cargo, QEMU, firmware CODE/VARS e
   perfil de máquina. Conservar os argumentos efetivamente usados.
3. Log de construção e os resultados de host/integração realmente executados,
   com status de saída. Um título `APROVADO` copiado de outro ciclo não serve.
4. Imagem, EFI e hashes relacionados a essa construção. Se conservar ELFs
   intermediários para depuração, identifique que pertencem ao mesmo build.
5. Resumo que diga o que passou, o que falhou, o que não foi executado e quais
   limitações permanecem. Resultados históricos devem continuar identificados.

Não copiar dependências privadas do ambiente, credenciais ou diretórios de
toolchain para o pacote. O produto precisa de seus fontes e artefatos declarados;
QEMU e OVMF são obtidos separadamente.

### Identificadores históricos dos binários 0.2

| Artefato histórico | Bytes | SHA-256 registrado |
| --- | ---: | --- |
| `saida/raiz-os.img` | 134217728 | `e3d637664af0a65af430b0696db1f522540bd21a72a5edc178980f367779e60e` |
| `saida/esp/EFI/BOOT/BOOTX64.EFI` | 781824 | `b6d949e6fa4bd4f7cb007018818d3dcce5e567336eb26caeff356dbb45b81947` |

Fonte dos identificadores históricos:
[resumo da validação](../../evidencias/RESUMO-VALIDACAO.md). O
[manifesto SHA256SUMS](../../evidencias/SHA256SUMS) acompanha o conteúdo do pacote
entregue: fontes, documentos, logs e os dois binários 0.2 preservados. Sua
atualização no empacotamento não representa uma nova execução dos testes.
Para comparar bytes locais em um host com `sha256sum`:

```sh
sha256sum saida/raiz-os.img saida/esp/EFI/BOOT/BOOTX64.EFI
```

Compare o resultado com a referência apropriada. Na formalização, o manifesto
do pacote é regenerado para incluir a documentação nova e conferir que os hashes
dos binários continuam iguais aos históricos. Exclua do manifesto o próprio
arquivo, os dados Git, `target/` e o estado mutável da execução, como OVMF VARS.
Uma alteração documental pode mudar os hashes de documentos sem mudar a imagem
executável ou sua evidência histórica.

Depois de uma nova construção, associe o manifesto aos binários e à execução
correspondentes. Preserve os identificadores e os registros anteriores; não
reescreva resultados históricos para fazer uma comparação passar. A versão do
manifesto identifica o conteúdo de uma entrega, enquanto os relatórios indicam
quais testes foram efetivamente executados sobre cada binário.

Um hash identifica bytes, mas não demonstra autenticidade de uma publicação,
correção do programa ou compilação reproduzível bit a bit. Essas afirmações
precisam de mecanismos e ensaios próprios.

## 6. Revisão de mudança e entrega

Uma proposta de alteração ou PR deve explicar o problema concreto, os componentes
afetados, o comportamento resultante e os limites. Referencie os IDs do backlog,
os contratos/ADRs relevantes e os comandos realmente executados. Informe quando
uma verificação foi dispensada por ser uma mudança somente documental.

Antes de marcar uma implementação como concluída:

- O critério de aceite correspondente deve estar satisfeito, com evidência
  localizável e ligada à versão correta.
- Os erros esperados, limites de filas/recursos e efeitos de morte/reinício
  precisam estar documentados quando a mudança afeta esses caminhos.
- Alterações incompatíveis na ABI precisam de versão e reconstrução coordenada;
  a aprovação histórica da ABI 2 não aprova novos códigos de chamada.
- A verificação deve alcançar o risco alterado. Não ampliar a alegação para
  hardware, desempenho, DMA ou casos que o teste não cobriu.

Uma entrega de versão reúne código, contratos, changelog, imagem, EFI, resumo,
logs selecionados e hashes da mesma construção. Antes de publicar uma nova
versão executável, realizar os gates aplicáveis ao conjunto de mudanças desde
a última base. A entrega de documentação de projeto pode conservar os binários
e resultados históricos 0.2, identificando essa opção explicitamente.

### Autoridade para ações externas

Preparar arquivos, revisar mudanças e executar verificações locais autorizadas
faz parte do trabalho do projeto. Criar um repositório remoto, enviar commits,
publicar release/imagem ou alterar permissões de acesso exige destino e escopo
autorizados pelo responsável. Uma autorização já fornecida na sessão deve ser
respeitada, sem pedir confirmação repetida para a mesma ação.

Formalizar este projeto não define por si só um remoto ou audiência de
publicação. As instruções e o pacote local podem ficar completos e revisáveis
antes dessa decisão. Não inserir credenciais em exemplos, logs ou arquivos de
configuração de entrega.

## 7. Como manter o backlog

[BACKLOG.csv](BACKLOG.csv) usa UTF-8 sem BOM, vírgula como separador e aspas
duplas para os campos. As colunas `dependencias` e `requisitos` contêm IDs
separados por ponto e vírgula. `dependencias` referencia tarefas `RZ-001` etc.;
`requisitos` relaciona cada tarefa aos IDs `RQ-001` a `RQ-018` do
[PRD](PRD.md). Esse vínculo indica contribuição ao requisito; uma tarefa
concluída não demonstra, por si só, a aceitação de todo o requisito.
Importe IDs, marcos e status como texto; `0.3` é um marco,
não uma data ou quantidade. Não há fórmulas, estimativas de horas ou datas de
entrega inventadas.

| Status | Significado |
| --- | --- |
| `concluido` | Critério satisfeito no escopo declarado, com evidência indicada. As linhas 0.2 referem-se ao resultado histórico, não a uma nova aprovação. |
| `proposto` | Trabalho descrito, ainda sem conclusão. Pode depender de decisão, desenho ou implementação anterior. |
| `pronto` | Trabalho ainda não iniciado, mas critério e escopo claros, decisões necessárias aceitas e todas as dependências verificadas. Não significa implementado. |
| `em_andamento` | Execução iniciada, com progresso referenciado e pendências explícitas. |
| `bloqueado` | Há impedimento concreto identificado; registrar qual condição deve mudar para continuar. |

O [ensaio controlado de idle](../../evidencias/execucao-rz-014-20261115/final/RELATORIO-TESTE-IDLE.md) foi aprovado em QEMU. Execute `cargo raiz compilar --teste-idle` e `cargo raiz testar-idle` para reproduzir o cenário opt-in com imagem separada. O despertar é provocado pela feature de teste, sem demonstrar evento real dos serviços. A ABI permanece 2.

Na execução de RZ-014 foram aprovados `cargo fmt --all --check`, os testes dos pacotes `protocolos`, `ferramentas`, `contrato-boot` e `contrato-sistema` (17 testes), `cargo test -p nucleo --lib` (11 testes), as compilações normal e com `--teste-idle`, e os comandos `testar-idle`, `testar`, `testar-isolamento` e `testar-recuperacao`. As primeiras tentativas de regressão tiveram comandos seriais truncados com envio em rajada ou a 5 ms por byte; a repetição com 50 ms por byte passou. Os registros ficam em [resumo da execução](../../evidencias/execucao-rz-014-20261115/RESUMO.md). O despertar do cenário instrumentado ocorre artificialmente no terceiro IRQ0.

Um ensaio posterior usou `cargo raiz compilar --teste-ipc-bloqueante` e `cargo raiz testar-ipc-bloqueante`: terminal e supervisor em CPL 3 exercitam despertar por envio IPC admitido em imagem opt-in. A execução incluiu testes host, builds normal e de teste e as três regressões; os resultados e ambiente estão no [resumo da execução IPC](../../evidencias/execucao-ipc-20261115/RESUMO.md). Esse ensaio usou chamada privada e não valida o binário posterior com a extensão ABI 2 da [ADR 004](../ADR-004-ESPERA-IPC-ABI-2.md). O retorno indica disponibilidade, não entrega garantida: a mensagem fica na fila e pode ser purgada antes de `RECEBER`. Não demonstra espera multiplexada, prazo, IRQ, cancelamento remoto ou segurança de eventos atrasados entre gerações.

A execução posterior da extensão ABI 2 passou em formatação, 18 testes dos pacotes host e 15 do núcleo, builds normal e instrumentados, `testar-ipc-bloqueante`, `testar-idle`, `testar`, `testar-isolamento` e `testar-recuperacao`, após reconstrução. Os registros e limites estão no [resumo da execução do supervisor](../../evidencias/execucao-supervisor-20261115/RESUMO.md). O ensaio de IPC usa serviços CPL 3 instrumentados; o de idle bloqueia artificialmente os serviços e os desperta no IRQ0, sem demonstrar IRQ de dispositivo. A combinação de kernel antigo com serviço novo não foi ensaiada em VM.

A execução de RZ-015 passou em formatação, 19 testes dos pacotes host e 17 do núcleo, builds normal e de recepção, ensaio `testar-receber-bloqueante`, três regressões normais e ensaios anteriores de IPC e idle. O ensaio em QEMU verificou mensagem já pendente com código igual a `CHAMADA` e chegada após bloqueio. A primeira tentativa falhou por parser que não aceitava prompt antes do marcador; foi preservada, o parser corrigido e a repetição passou. Revogação entre despertar e seleção foi testada no host, não em VM. Docker estava inativo; usou-se QEMU. Kernel antigo com serviço novo não foi testado em VM. Os registros serão organizados no [resumo RZ-015](../../evidencias/execucao-rz-015-20261115/RESUMO.md).

A execução de RZ-016 passou em formatação, 20 testes dos pacotes host e 21 do núcleo, build e verificação da imagem normal, `testar-prazo`, `testar-prazo-idle`, cinco boots de regressão e os testes anteriores de recepção, IPC e idle. Em QEMU, o cenário de prazo no idle observou três serviços bloqueados e timeout em CPL 0. Empate evento/timeout, volta do contador e substituição de instância foram testados no host, não em VM; fallback em kernel anterior também não foi ensaiado em VM. O supervisor normal ainda não usa a chamada com prazo. Os registros serão organizados no [resumo RZ-016](../../evidencias/execucao-rz-016-20261115/RESUMO.md).

A execução RZ-017 passou em formatação, 22 testes dos pacotes host e 23 do núcleo, builds normal e instrumentados de ciclo/observação, cinco boots de regressão e repetição dos ensaios de prazo, prazo-idle e recepção. Em QEMU, o ciclo verificou cancelamentos por geração, que o prazo antigo não acordou a nova instância, e a recusa por quota preservou o prazo original até timeout. O ensaio de observação verificou recusa de autoridade ao terminal, falha da rede e retorno `ALVO_INDISPONIVEL` ao supervisor, referência antiga após reinício e recuperação. Falha de preparação simulada e empate timeout/invalidação foram testados no host; observação perante substituição e fallback de kernel antigo não foram testados em VM. Os registros estão no [resumo RZ-017](../../evidencias/execucao-rz-017-20261115/RESUMO.md).

RZ-018 está concluído no recorte supervisor→rede: a [ADR 008](../ADR-008-NOTIFICACOES-VIDA.md) implementa um slot persistente supervisor→rede, separado da fila IPC, e espera de IPC/vida com prazo. Nesta etapa passaram 27 testes do núcleo e 3 do contrato no host, `verificar-imagem`, o [ensaio de vida em QEMU](../../evidencias/teste-vida-1794724542137348500/resultado.txt) e as regressões `testar`, `testar-isolamento` e `testar-recuperacao`. Uma tentativa inicial da fixture falhou e foi corrigida; o histórico está detalhado no resumo da execução. Os cinco ensaios específicos anteriores reconstruídos passaram, conforme o [resumo da execução](../../evidencias/execucao-rz-018-20261115/RESUMO.md); as regressões gerais também passaram novamente na imagem normal final. O supervisor automático ainda consulta estado e prazos por polling; IRQ e espera multiplexada completa permanecem pendentes.

RZ-019 está concluído no recorte implementado: a [ADR 009](../ADR-009-LIMITES-IPC.md) registra caixas de 16 mensagens, limite de 15 por origem lógica na caixa do terminal e slots estruturais de inscrição e espera por processo. `cargo test -p nucleo --lib` passou com 29 testes e `cargo test -p contrato-sistema` com 3 testes no host; a construção normal e a verificação da imagem passaram. As regressões normais em QEMU passaram. RZ-020 está concluído no recorte de IRQ4 exclusiva. O [ensaio de quota em QEMU](../../evidencias/teste-quota-1794725073649578100/resultado.txt) passou para 15 envios da rede, recusa do 16º, reserva ao supervisor, FIFO e readmissão após drenagem. A ordem inversa e liberação após uma retirada ou purga foram testadas no host, não nesse ensaio de VM. O [resumo da execução](../../evidencias/execucao-rz-019-20261115/RESUMO.md) reúne construção normal, verificação de imagem e os cinco boots de regressão.


RZ-020 está concluído no recorte de IRQ4 exclusiva: a [ADR 010](../ADR-010-NOTIFICACAO-IRQ4.md) acrescenta à ABI 2 uma associação exclusiva de IRQ4 ao terminal, notificação coalescida separada da fila IPC e confirmação por sequência. No [ensaio opt-in em QEMU](../../evidencias/teste-irq-1794725963506321900/resultado.txt), IRQ4 acordou o terminal a partir de idle CPL 0; foram observadas negação a outro serviço, recusas de sequência incorreta e duplicada, rechecagem e rearme. Com B pendente e sem confirmação por 50 tiques, a máscara foi observada em duas amostras, o contador de entradas permaneceu igual e o timer avançou; C chegou após o rearme. Coalescência foi testada no host, não demonstrada nesse ensaio QEMU. Isso não demonstra ausência absoluta de tempestades nem suporte a hardware físico. A construção normal, verificação de imagem e três comandos de regressão em QEMU (cinco boots) passaram. Naquela execução o terminal normal ainda usava polling. SMP e IRQ compartilhada não foram demonstrados. Os ensaios reconstruídos de quota, vida e prazo-idle também passaram; o [resumo da execução RZ-020](../../evidencias/execucao-rz-020-20261115/RESUMO.md) reúne logs, hashes e limites.

RZ-013 está concluído como decisão documental na [ADR 003](../ADR-003-ESPERA-0.3.md). RZ-014 está concluído no escopo de estados, despertar e idle; RZ-015 está concluído para recepção da própria caixa; RZ-016 está concluído para prazo de recepção IPC; RZ-017 está concluído no recorte de cancelamento e observação pontual; RZ-018 está concluído no recorte supervisor→rede; o marco 0.3 e a espera multiplexada completa permanecem pendentes na [especificação](ESPECIFICACAO-0.3.md). O piloto de interrupção proposto é **UART COM1/IRQ4
exclusiva**; NIC e PS/2 continuam por polling nesse recorte.

Para promover um item a `pronto`, confira cada dependência pelo ID e sua
evidência, resolva decisões abertas e registre um aceite verificável. Para
promovê-lo a `concluido`, acrescente a evidência da execução ou revisão que
satisfaz o aceite. Conservar IDs ao editar títulos ou reorganizar marcos;
não reutilizar IDs de itens removidos para outro trabalho.

## 8. Limites que acompanham a entrega

A base 0.2 tem três serviços incorporados, uma CPU, permissões estáticas e
recuperação apenas da rede, com quatro encarnações por boot. O driver DMA ainda
precisa ser confiável. Quarentena de arenas e proteção de CPU não demonstram
confinamento por IOMMU ou ausência de todas as transações de dispositivo em voo.

Ainda faltam espera multiplexada, notificações gerais,
memória/processos dinâmicos, capabilities delegáveis, arquivos persistentes,
TCP/DHCP/DNS, aplicações independentes e suporte físico declarado. A ESP FAT32 é
para o boot; não é um serviço de arquivos disponível ao SO após o boot.
O histórico não é benchmark, certificação ou prova de tempo real.

Essas fronteiras são detalhadas em [ARQUITETURA.md](../ARQUITETURA.md),
[ADR-002-CICLO-DE-VIDA.md](../ADR-002-CICLO-DE-VIDA.md) e
[ABI-0.2.md](../ABI-0.2.md).


RZ-021 está concluído no recorte validado. O terminal normal agora usa IRQ4/IPC e prazo para PS/2, conforme a [ADR 011](../ADR-011-TERMINAL-UART.md). O [ensaio instrumentado](../../evidencias/teste-uart-1794726895277854900/resultado.txt) passou, inclusive na janela de rearme controlada; o ensaio normal, construção, verificação de imagem e cinco boots normais também passaram. Os ensaios adicionais de IRQ, quota, vida e prazo-idle passaram; no host passaram 31 testes do núcleo e 23 dos pacotes. O PS/2 emulado por HMP executou o comando sobre e recebeu resposta distinta. As rajadas ensaiadas foram de quatro bytes a cada 50 ms. PS/2 físico e fallback com kernel antigo não foram validados em VM; não há garantia de perda zero geral. RZ-022 foi concluído posteriormente, conforme a [ADR 012](../ADR-012-ESPERA-SERVICOS-RZ022.md); RZ-023 foi concluído no escopo do aceite original, com lacunas T03 descritas no [resumo](../../evidencias/execucao-rz-023-20261115/RESUMO.md) e na [matriz](../../evidencias/execucao-rz-023-20261115/MATRIZ.md). RZ-024 foi concluído no recorte controlado, conforme o [resumo](../../evidencias/execucao-rz-024-20261115/RESUMO.md). Os itens seguintes permanecem propostos e o marco 0.3 segue aberto.

## 9. Andamento de RZ-022

RZ-022 está **concluído no escopo validado**, sob ABI 2. A [ADR 012](../ADR-012-ESPERA-SERVICOS-RZ022.md)
e o [resumo da execução](../../evidencias/execucao-rz-022-20261115/RESUMO.md)
reúnem a integração final, regressões e limites. O [resultado opt-in integrado](../../evidencias/execucao-rz-022-1794731439514596800/resultado.txt)
registra dois boots aprovados:

- Recuperação manual: após reset, 16 mensagens foram guardadas na FIFO local
  do serviço e outras 16 ficaram na caixa IPC do kernel; a 33ª foi recusada
  com `FILA_CHEIA`. Após o prazo de 500 tiques, os ACKs 1 a 32 chegaram em
  ordem. O modo manual não reiniciou a rede após UD2; o comando manual criou a
  geração 2, que respondeu a ping e UDP.
- Recepção de rede: o serviço anunciou e aguardou seu prazo de polling RX de
  10 tiques; o ensaio observou timeout antes do eco UDP idêntico e do aumento
  dos contadores RX/UDP. A ordem do ensaio não determina o instante físico em
  que o datagrama entrou no dispositivo.
- Recuperação automática: três tentativas foram observadas com diferenças de
  100, 200 e 400 tiques; as gerações 2, 3 e 4 responderam a ping e UDP.

Após o reforço do harness, o teste separado do ciclo de espera passou duas vezes:
[primeira execução](../../evidencias/teste-ciclo-espera-1794731367103979200/resultado.txt)
e [segunda execução](../../evidencias/teste-ciclo-espera-1794731396374947500/resultado.txt).
Também passaram a compilação/verificação da imagem normal, smoke, isolamento,
recuperação e os ensaios complementares listados no resumo. A validação registrou
`cargo fmt --all --check`, 28 testes dos pacotes host e 31 testes do núcleo.
Tentativas anteriores, inclusive as malsucedidas, foram preservadas nos diretórios
originais e não foram substituídas pelos resultados finais.

RZ-023 foi concluído no escopo do aceite original, com 12 comandos opt-in após
rebuild, testes host e cinco boots normais; consulte o [resumo](../../evidencias/execucao-rz-023-20261115/RESUMO.md)
e a [matriz T03](../../evidencias/execucao-rz-023-20261115/MATRIZ.md), que
preserva evidências, falhas e lacunas por cenário. RZ-024 foi concluído no
recorte controlado, com observações finitas de IRQ4, prazo e pressão, conforme o
[resumo](../../evidencias/execucao-rz-024-20261115/RESUMO.md). Isso não conclui
todos os T03 nem o marco 0.3. Não há validação em VM da compatibilidade com
kernel ABI 2 anterior; a e1000 continua em polling e os resultados não
estabelecem perda zero ou desempenho em hardware físico.
