# Imagem inicializável do Raiz OS 0.1

O comando `cargo run -p ferramentas -- compilar` compila primeiro os serviços
`servico-terminal` e `servico-rede` para `x86_64-unknown-none`. Em seguida compila
o microkernel para o mesmo alvo, fornecendo os caminhos absolutos dos ELFs pelas
variáveis `RAIZ_TERMINAL` e `RAIZ_REDE`. Por último compila o carregador para
`x86_64-unknown-uefi`, com `RAIZ_KERNEL` apontando para o ELF do microkernel.
O resultado é um único arquivo de distribuição: `saida/raiz-os.img`.

O microkernel, os serviços e o carregador pertencem ao mesmo produto, mas mantêm
formatos, privilégios e responsabilidades distintos. A imagem contém
`EFI/BOOT/BOOTX64.EFI`; esse arquivo PE incorpora o ELF do microkernel, que
transporta os ELFs dos serviços. O firmware executa o carregador, o carregador
transfere o controle ao microkernel, e este prepara os serviços em espaços de
endereçamento próprios e modo usuário. Incorporar os bytes em um arquivo de
distribuição não coloca os serviços no mesmo nível de privilégio do kernel.

## Layout de disco

| Região | LBA inicial | LBA final | Conteúdo |
| --- | ---: | ---: | --- |
| MBR protetor | 0 | 0 | Partição tipo `0xEE`; assinatura `55 AA` |
| GPT principal | 1 | 1 | Cabeçalho de 92 bytes, revisão 1.0 e CRC32 |
| Entradas GPT principais | 2 | 33 | 128 entradas de 128 bytes |
| Espaço de alinhamento | 34 | 2047 | Zeros |
| ESP | 2048 | 262110 | FAT32 |
| Entradas GPT de reserva | 262111 | 262142 | Cópia das entradas principais |
| GPT de reserva | 262143 | 262143 | Cabeçalho e CRC32 próprios |

A imagem tem 262.144 setores de 512 bytes, totalizando 128 MiB. A ESP começa
em 1 MiB. O construtor é Rust com `std` e não precisa de `mkfs`, `mount`,
privilégios administrativos ou acesso a um disco físico.

Os GUIDs de disco e partição são determinísticos nesta edição de laboratório.
Um instalador futuro deve gerar identificadores únicos por instalação.

## FAT32

- Um setor por cluster, 32 setores reservados e duas FATs espelhadas.
- FATs dimensionadas conforme o número de clusters; pelo menos 65.525 clusters
  de dados para caracterizar FAT32.
- Diretório raiz no cluster 2, `EFI` no cluster 3 e `BOOT` no cluster 4.
- `BOOTX64.EFI` começa no cluster 5 e possui cadeia FAT com término explícito.
- FSInfo no setor relativo 1, cópia no 7; boot sector no 0, cópia no 6.
- Nomes curtos 8.3, sem entradas de nomes longos. Datas fixas de diretório
  permitem reproduzir a imagem com os mesmos bytes do carregador.
- Nenhum boot BIOS legado é implementado.

O FAT32 da imagem serve para o **firmware encontrar o carregador**. Ele não
equivale a um driver FAT32 disponível no kernel após `ExitBootServices`.

## Verificação

`cargo run -p ferramentas -- verificar-imagem` confere o tamanho do disco,
MBR protetor, os dois cabeçalhos GPT e seus CRCs, as cópias das entradas GPT,
o tipo e a posição da ESP, os parâmetros FAT32, FSInfo, as FATs espelhadas e
os diretórios. Em seguida percorre a cadeia do arquivo e compara todos os bytes
com `saida/esp/EFI/BOOT/BOOTX64.EFI`.

Essa verificação confirma o formato gravado. O boot em QEMU continua sendo
uma verificação separada e necessária para afirmar que o sistema iniciou.

## Execução

```text
cargo run -p ferramentas -- executar
cargo run -p ferramentas -- executar --grafico
```

O perfil utiliza QEMU/TCG, máquina `pc`, um processador, 256 MiB de memória,
firmware UEFI OVMF, disco IDE com alterações temporárias e placa E1000. O
console serial usa a entrada/saída do terminal do host. `Ctrl+C` encerra o
emulador. O modo `--grafico` habilita a janela de vídeo; o console serial
permanece disponível.

| Variável | Finalidade |
| --- | --- |
| `RAIZ_QEMU` | Caminho do executável `qemu-system-x86_64` |
| `RAIZ_QEMU_DADOS` | Diretório de ROMs/dados do QEMU, passado com `-L` |
| `RAIZ_OVMF` | Arquivo `OVMF_CODE.fd` ou `OVMF_CODE_4M.fd` |
| `RAIZ_OVMF_VARS` | Arquivo VARS correspondente ao CODE utilizado |

O arquivo VARS é copiado para `saida/OVMF_VARS.fd` na primeira execução.
Não misture CODE e VARS de geometrias diferentes. A ferramenta também procura
firmware nos caminhos usuais do host. Para usar outros pacotes, informe os
caminhos explicitamente. Firmware e emulador são ferramentas externas de
teste; não estão incorporados ao código do Raiz OS.

A rede de laboratório usa `10.0.2.15` e encaminha UDP de
`127.0.0.1:17777` no host para `10.0.2.15:7777` no guest. O endereço de
loopback evita publicar o serviço de eco na rede externa do host.

## Cliente UDP e teste de integração

Com uma VM já aberta, um segundo terminal pode executar:

```text
cargo raiz eco-udp ola
```

O cliente envia o texto para `127.0.0.1:17777`, aguarda até três segundos e
confere se a resposta é idêntica. O limite de payload do laboratório é 1472
bytes; o cliente não implementa fragmentação IPv4.

Para iniciar uma VM de teste, observar seu boot e encerrá-la automaticamente:

```text
cargo raiz testar
```

Encerre a VM interativa antes, pois ambas utilizam a porta UDP 17777. O teste
usa as mesmas variáveis de QEMU/OVMF e copia VARS para um arquivo separado,
`saida/OVMF_VARS_TESTE.fd`. A imagem existente é verificada; somente sua ausência
faz a ferramenta chamar a compilação.

O roteiro observa `EXIT_BOOT_SERVICES_OK`, `TERMINAL_PRONTO` e o serviço de rede
em `CPL=3`. Confere a resposta de `sobre` indicando o terminal em `CPL=3`, um
ping ao gateway e eco UDP com payloads de 0, 1, 17 e 1472 bytes. Envia mais 80
datagramas sequenciais para percorrer os anéis RX/TX e confere os contadores
de `rede`.

Depois verifica a negação de uma operação sem capacidade, provoca uma instrução
inválida no serviço de rede com `falhar-rede` e aguarda o diagnóstico de falha
do serviço 2, vetor 6. Para considerar o caso aprovado, o terminal deve responder
a um novo comando `eco`, `processos` deve indicar `rede=2` e um novo envio ao
serviço de rede deve produzir o marcador de erro IPC esperado.

Esse caso comprova somente a contenção observada de uma falha de CPU no serviço
de rede e a continuidade do terminal/microkernel na plataforma de teste. **Não
comprova contenção de DMA, IOMMU, reinício automático de serviços nem isolamento
completo para produção.** O caso de instrução inválida não testa a MMU nem a
preempção; o comando separado abaixo cobre dois cenários específicos.

Cada execução grava `evidencias/serial-smoke.log`,
`evidencias/qemu-smoke-stderr.log` e `evidencias/RELATORIO-SMOKE.md`. O relatório
diz quais etapas foram concluídas, quantas respostas foram verificadas e a
falha observada, se houver. O processo do emulador é encerrado mesmo quando
uma etapa falha. Um relatório aprovado refere-se somente aos casos executados
e à plataforma virtual declarada.

## Proteção de memória e preempção

```text
cargo raiz testar-isolamento
```

Esse comando inicializa duas VMs sequencialmente, encerrando cada uma antes de
iniciar a seguinte. Usa a imagem existente, as mesmas variáveis de QEMU/OVMF e
a porta 17777. Não execute uma VM interativa simultaneamente.

No primeiro boot, `violar-memoria` solicita ao serviço de rede uma leitura do
endereço `0x02000000` do núcleo. O teste exige page fault (vetor 14) do serviço 2
com `CR2=0x2000000`. Depois exige uma resposta completa do terminal ao comando
`eco MEMORIA_PROTEGIDA_TERMINAL_VIVO` e os estados `terminal=1 rede=2`.

No segundo boot, `ocupar-rede` coloca o serviço de rede em loop sem cessão
voluntária. Após observar `[rede] LOOP_SEM_CEDER`, o teste espera 200 ms, solicita
`eco PREEMPCAO_OK` e confere a resposta. Também consulta `processos` duas vezes,
com intervalo de 200 ms, exigindo os dois serviços ativos e aumento dos tiques.
Isso verifica que o terminal obtém CPU nesse cenário mesmo com o outro serviço
sem ceder voluntariamente.

O relatório fica em `evidencias/RELATORIO-ISOLAMENTO.md`. Os logs possuem nomes
separados: `serial-isolamento-memoria.log`, `qemu-isolamento-memoria-stderr.log`,
`serial-isolamento-preempcao.log` e `qemu-isolamento-preempcao-stderr.log`. O
relatório e os logs do comando `testar` são preservados.

Os casos têm limites explícitos: o teste de leitura não cobre DMA/IOMMU nem todos
os endereços ou tipos de acesso; o teste de preempção não mede latência máxima
nem estabelece garantia de tempo real. Nenhum dos dois afirma reinício automático
do serviço ou isolamento completo para produção.

## Fontes do formato

- [UEFI 2.10 — GPT Disk Layout](https://uefi.org/specs/UEFI/2.10/05_GUID_Partition_Table_Format.html).
- [UEFI — Protocols: Media Access](https://uefi.org/specs/UEFI/2.10/13_Protocols_Media_Access.html).
- [Microsoft Windows driver samples — definições FAT/FSInfo](https://github.com/microsoft/Windows-driver-samples/blob/main/filesys/fastfat/fat.h).
- [QEMU — Network emulation](https://www.qemu.org/docs/master/system/devices/net.html).

A implementação do construtor foi escrita para este projeto; ela segue os
formatos documentados. Não contém código de filesystem de outro kernel.
