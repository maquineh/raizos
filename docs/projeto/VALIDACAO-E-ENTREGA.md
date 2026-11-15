# Validação e entrega do Raiz OS

Este documento define como selecionar verificações, conservar evidências e
preparar uma entrega. A base técnica é a versão experimental **0.2.0**. O próximo
marco, **0.3**, está em planejamento: escrever sua proposta não significa aprovar
a ABI ou implementar IPC bloqueante.

Os resultados abaixo são **históricos da validação de 0.2**, cuja data de
referência registrada é 9 de novembro de 2026, America/Boa_Vista. A redação e a
revisão destes documentos não executaram novamente testes Rust, compilação do SO
ou VMs. A integridade dos arquivos do pacote é conferida separadamente durante
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

No planejamento inicial, nenhum item de implementação de 0.3 está `pronto` ou
concluído. A proposta revisável está disponível em
[ESPECIFICACAO-0.3.md](ESPECIFICACAO-0.3.md), mas a decisão da ABI e a implementação
permanecem pendentes. O piloto de interrupção proposto é **UART COM1/IRQ4
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

Ainda faltam IPC bloqueante, timeouts associados a espera, notificações gerais,
memória/processos dinâmicos, capabilities delegáveis, arquivos persistentes,
TCP/DHCP/DNS, aplicações independentes e suporte físico declarado. A ESP FAT32 é
para o boot; não é um serviço de arquivos disponível ao SO após o boot.
O histórico não é benchmark, certificação ou prova de tempo real.

Essas fronteiras são detalhadas em [ARQUITETURA.md](../ARQUITETURA.md),
[ADR-002-CICLO-DE-VIDA.md](../ADR-002-CICLO-DE-VIDA.md) e
[ABI-0.2.md](../ABI-0.2.md).
