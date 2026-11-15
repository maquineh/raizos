# Raiz OS 0.1.0 — resultado da validação

Registro histórico da execução 0.1. Os resultados e hashes abaixo pertencem à
imagem daquela versão; os links de documentação apontam para a documentação atual.

## Resultado

**Compilação concluída, 13 testes no host aprovados e três inicializações de
integração em QEMU aprovadas.** A classificação adotada é SO experimental mínimo
com microkernel próprio. O alcance é a plataforma e os cenários abaixo.

## Plataforma e ferramentas observadas

| Item | Valor |
|---|---|
| Rust | `rustc 1.99.1 (58cf8f9c3 2026-10-13)` |
| Cargo | `cargo 1.99.1 (5f94df478 2026-08-27)` |
| Alvos | `x86_64-unknown-none` e `x86_64-unknown-uefi` |
| Emulador | QEMU 10.0.13, Debian `1:10.0.13+ds-0+deb13u1` |
| Firmware | OVMF, par CODE/VARS de 4 MiB |
| Máquina | `pc` / i440fx, TCG, uma CPU, 256 MiB |
| NIC | e1000/Intel 82540EM emulada, PCI 8086:100e |
| Rede | IP fixo 10.0.2.15, gateway virtual 10.0.2.2 |

Firmware, emulador e compilador são ferramentas/dependências da plataforma e não
estão incorporados no código do SO nem no pacote de entrega.

## Verificações concluídas

| Verificação | Resultado observado |
|---|---|
| Formatação Rust | `cargo fmt --all --check` concluído |
| Protocolos | Nove testes aprovados |
| Ferramentas | Quatro testes aprovados |
| Construção | Serviços ELF, microkernel ELF, carregador EFI e imagem de disco produzidos sem avisos na execução final |
| Imagem | GPT primária/secundária, CRCs, FAT32 e conteúdo EFI verificados pelo construtor |
| Boot | `EXIT_BOOT_SERVICES_OK`; terminal e rede em CPL 3 |
| Rede | ICMP ao gateway recebido; 84 respostas UDP comparadas byte a byte, 6.610 bytes de payload |
| Autoridade | Terminal recebe recusa ao tentar a operação PCI atribuída à rede |
| Exceção de usuário | Instrução inválida da rede, vetor 6; terminal continua e IPC ao destino morto falha como esperado |
| Proteção de memória | Leitura de `0x02000000` pela rede gera page fault, vetor 14, erro 0x5; terminal continua |
| Preempção | Rede fica em loop sem ceder; terminal responde `PREEMPCAO_OK`; ambos seguem executáveis e contador de timer avança |

Os crates de contratos participam do comando de teste, mas não contêm testes
unitários próprios; não estão contabilizados no total de 13.

## Reprodução

Com Rust, QEMU e OVMF instalados, a partir da raiz do projeto:

```sh
cargo fmt --all --check
cargo test -p protocolos -p ferramentas -p contrato-boot -p contrato-sistema
cargo raiz compilar
cargo raiz testar
cargo raiz testar-isolamento
```

O arquivo `rust-toolchain.toml` fixa a versão de Rust e os alvos. Se necessário,
configure `RAIZ_QEMU`, `RAIZ_OVMF` e `RAIZ_OVMF_VARS` conforme o README.
`testar` e `testar-isolamento` usam a imagem existente; recompile após alterações.

## Artefatos desta execução

| Arquivo | Tamanho em bytes | SHA-256 |
|---|---:|---|
| `saida/raiz-os.img` | 134217728 | `2e8bc46ba873955cd5955cb707d0d9a13e505d51742a13382ccd3281b56141c6` |
| `saida/esp/EFI/BOOT/BOOTX64.EFI` | 652800 | `d57c908420ed958841e0f134fd48a8a103a188253f70c44aa83f59a74652e840` |

A imagem tem 128 MiB antes de compressão. Seu tamanho não representa a quantidade
de código do kernel. O arquivo EFI incorpora o núcleo e os dois serviços.

## Evidências detalhadas

- [Integração e rede](RELATORIO-SMOKE.md).
- [Memória e preempção](RELATORIO-ISOLAMENTO.md).
- Logs `serial-smoke.log`, `serial-isolamento-memoria.log` e
  `serial-isolamento-preempcao.log` no mesmo diretório.
- [Matriz baseada nos conceitos de Tanenbaum](../../docs/CRITERIOS-TANENBAUM.md).
- [Arquitetura e limitações](../../docs/ARQUITETURA.md).

## Limites do resultado

Os testes comprovam os casos descritos em uma configuração virtual. Não há
validação em hardware físico, benchmark de desempenho, garantia de tempo real,
prova geral de isolamento, IOMMU, reinício automático de serviços, filesystem
persistente, execução dinâmica de aplicações ou rede TCP/DHCP/DNS nesta versão.
O driver autorizado a DMA ainda precisa ser confiável. A imagem não é um
instalador para computadores físicos nem um produto pronto para uso cotidiano.
