# Desenvolver o Raiz OS

O ponto de partida é a versão 0.2.0 existente. A primeira evolução planejada está
na [especificação 0.3](docs/projeto/ESPECIFICACAO-0.3.md), ainda proposta.

## 1. Abrir a base

Extraia o pacote em uma pasta de trabalho e abra a pasta `raiz-os` no editor.
O [PROJETO.md](PROJETO.md) relaciona requisitos, decisões, backlog e evidências.
O pacote conserva o histórico Git local; confira a importação:

```sh
git status
git log -1 --oneline
git tag --list
```

A branch inicial é `main`, com a marca `projeto-base-0.2.0`. O commit inicial
registra a importação do código 0.2 e da documentação de projeto, sem simular
commits antigos. Usa a identidade técnica `Raiz OS <raiz-os@localhost>`;
esse endereço não representa uma pessoa ou canal de suporte. Configure sua
própria identidade local antes de criar os seus commits:

```sh
git config user.name "Seu nome"
git config user.email "seu-email"
```

Não há remoto configurado. O projeto pode ser desenvolvido localmente; a seleção
de um serviço de hospedagem pode ser feita quando houver um destino definido.

## 2. Preparar as ferramentas

O fluxo de referência foi executado em um host Linux com Rust/rustup, QEMU
x86-64 e OVMF. O sistema convidado é o Raiz OS. Não precisa existir Linux ou
outro SO instalado dentro da imagem.

Instale Rust por rustup. `rust-toolchain.toml` fixa a versão usada na base e
solicita os alvos `x86_64-unknown-none` e `x86_64-unknown-uefi`. Confira o ambiente:

```sh
rustup show
rustc --version
cargo --version
```

Instale o emulador QEMU com suporte a x86-64 e um par correspondente de firmware
OVMF CODE/VARS. O relatório histórico usou a variante de 4 MiB. Quando o comando
ou os caminhos não forem encontrados automaticamente, configure-os:

```sh
export RAIZ_QEMU=/caminho/qemu-system-x86_64
export RAIZ_OVMF=/caminho/OVMF_CODE_4M.fd
export RAIZ_OVMF_VARS=/caminho/OVMF_VARS_4M.fd
```

Esses caminhos são exemplos e precisam existir na sua instalação. Não inclua o
firmware ou as ferramentas externas no código do sistema. O QEMU usado nos
ensaios está identificado no [resumo de validação](evidencias/RESUMO-VALIDACAO.md).

No Windows, extraia o pacote em uma pasta de desenvolvimento, por exemplo
`C:\Projetos\raiz-os`. A sintaxe para as variáveis no PowerShell é:

```powershell
$env:RAIZ_QEMU = 'C:\Ferramentas\qemu\qemu-system-x86_64.exe'
$env:RAIZ_OVMF = 'C:\Ferramentas\ovmf\OVMF_CODE_4M.fd'
$env:RAIZ_OVMF_VARS = 'C:\Ferramentas\ovmf\OVMF_VARS_4M.fd'
```

Os comandos Cargo abaixo são os mesmos, mas **o fluxo em Windows ainda não foi
validado**. Esses exemplos não significam que arquivos ou ferramentas já foram
instalados no seu computador. Registre eventuais ajustes de portabilidade em um
item próprio, sem declará-los suportados antes da execução.

## 3. Conferir, construir e executar

A imagem 0.2 incluída no pacote permite conferir seu formato e executar o sistema
antes de alterar o código:

```sh
cargo raiz verificar-imagem
cargo raiz executar
```

Para reconstruir a imagem a partir das fontes:

```sh
cargo raiz compilar
cargo raiz verificar-imagem
cargo raiz executar
```

`cargo raiz executar --grafico` abre a interface gráfica do emulador. No terminal
do SO, comece por `ajuda`, `processos`, `rede` e `ping 10.0.2.2`.
O construtor produz `saida/raiz-os.img`; os serviços permanecem em processos
separados. O diretório de saída não é fonte versionada.

## 4. Fazer uma alteração

1. Escolha um item do [backlog](docs/projeto/BACKLOG.csv) com dependências
   satisfeitas e um resultado verificável.
2. Crie uma branch de trabalho, por exemplo `git switch -c evolucao/0.3-ipc`.
3. Leia a especificação e feche os detalhes necessários do contrato antes de
   implementá-lo. Registre decisões arquiteturais em uma ADR.
4. Mude o código e os documentos afetados. Mantenha kernel e serviços alinhados
   quando a ABI mudar.
5. Execute a validação correspondente ao risco da mudança e conserve o resultado.
6. Atualize o item com evidências e limites; depois registre o commit com mensagem
   que descreva o problema resolvido e o comportamento produzido.

Para assistência de programação, as regras estão em [AGENTS.md](AGENTS.md).
O [roteiro de continuidade](docs/projeto/CONTINUIDADE.md) contém um contexto
pronto para a próxima sessão; a fonte e os relatórios precisam acompanhá-lo.

## 5. Escolher a validação

Para lógica testável no host:

```sh
cargo fmt --all --check
cargo test -p protocolos -p ferramentas -p contrato-boot -p contrato-sistema
cargo test -p nucleo --lib
```

Para alterações no comportamento executável do sistema:

```sh
cargo raiz compilar
cargo raiz verificar-imagem
cargo raiz testar
cargo raiz testar-isolamento
cargo raiz testar-recuperacao
```

Esses comandos de integração usam a imagem existente; recompilar é necessário
após alterar fontes do convidado. O processo de teste escreve em `evidencias/`;
preserve relatórios anteriores quando forem a base histórica de outra entrega.

Mudanças só de texto exigem verificar links, comandos e coerência. Não é preciso
repetir cinco boots por uma correção documental. A política completa e os casos
que exigem nova execução estão em
[Validação e entrega](docs/projeto/VALIDACAO-E-ENTREGA.md).

## 6. Preparar uma entrega

Mantenha a versão do código consistente com o que está implementado. Documentar
0.3 não promove o pacote para 0.3. Atualize o histórico, os limites e o manifesto
de integridade do pacote quando houver uma entrega nova.

O manifesto `evidencias/SHA256SUMS` cobre fontes, documentação, evidências e os
dois artefatos de boot incluídos. Ele não cobre a si mesmo, dados internos do Git,
cache de compilação ou arquivos mutáveis de execução. Confira os hashes a partir
da raiz do projeto; no host de referência:

```sh
sha256sum --check evidencias/SHA256SUMS
```

Uma imagem nova requer logs que a identifiquem. Uma reorganização documental
pode conservar os mesmos binários e apontar explicitamente para a evidência
histórica. Não acrescente ferramentas, firmware, credenciais, diretórios de cache
ou estado de NVRAM da VM ao pacote de distribuição.
