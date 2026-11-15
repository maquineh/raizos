# Raiz OS 0.2.0 — resultado da validação

**Data de referência:** 9 de novembro de 2026, America/Boa_Vista (UTC−04).

## Resultado

**Compilação concluída sem avisos, 26 testes no host aprovados e cinco
inicializações de integração em QEMU aprovadas.**

A versão acrescenta supervisor em ring 3, referências com geração e recuperação
limitada da rede. A classificação permanece SO experimental mínimo com
microkernel próprio; não há ambiente geral de aplicações ou persistência.

## Ambiente observado

| Item | Valor |
|---|---|
| Rust | `rustc 1.99.1 (58cf8f9c3 2026-10-13)` |
| Cargo | `cargo 1.99.1 (5f94df478 2026-08-27)` |
| Alvos | `x86_64-unknown-none` e `x86_64-unknown-uefi` |
| QEMU | 10.0.13, Debian `1:10.0.13+ds-0+deb13u1` |
| Firmware | OVMF, par CODE/VARS de 4 MiB |
| Máquina | `pc` / i440fx, TCG, uma CPU, 256 MiB |
| NIC | e1000/Intel 82540EM emulada, PCI 8086:100e |
| Serviços | Terminal, rede e supervisor, todos em CPL 3 |
| Rede | IP fixo 10.0.2.15, gateway virtual 10.0.2.2 |

Firmware, emulador e compilador são dependências de ferramentas/plataforma e não
estão incorporados no código do SO nem no pacote de entrega.

## Testes no host

| Área | Quantidade | Cobertura principal |
|---|---:|---|
| Protocolos | 9 | Tamanhos, truncamento, checksums, fragmentação/opções recusadas |
| Ferramentas | 5 | CRC, imagem, estados completos e cadeia de arenas sem reutilização/reboot |
| Contrato de serviços | 3 | Codificação de referências, valores inválidos e layout ABI 2 |
| Controle do núcleo | 9 | FIFO circular, purga seletiva, identidade, grafo IPC, autoridade e quota |
| **Total** | **26** | Todos aprovados |

O crate de contrato de boot participa do comando, mas não contém testes próprios
e não aumenta o total. Log integral: [testes-host-0.2.log](testes-host-0.2.log).

## Integração em cinco boots

| Cenário | Resultado observado |
|---|---|
| Integração de boot e rede | Imagem válida, boot próprio, ring 3, ICMP e 84 respostas UDP idênticas; permissão PCI negada ao terminal e continuidade após UD2 da rede |
| Memória | Rede tenta ler `0x02000000`; page fault vetor 14/erro 0x5; terminal continua |
| Preempção | Rede entra em loop sem ceder; terminal responde e contador avança de 53 para 83 |
| Ciclo de recuperação | Gerações 1→2→3→4; reinício manual, automático, parada/retomada; referências antigas e chamadas sem autoridade recusadas; quinta encarnação recusada com rede viva |
| Recuperação de loop | Rede executável, mas presa em laço, é reiniciada por pedido ao supervisor; ping e UDP voltam a responder |

No ciclo de recuperação foram conferidas quatro arenas físicas diferentes:

```text
0x249a000 → 0x269a000 → 0x289a000 → 0x2a9a000
```

No cenário de loop foram conferidas duas arenas diferentes. Cada cenário de
recuperação contém uma única ocorrência de `EXIT_BOOT_SERVICES_OK`; as trocas
ocorreram sem reboot daquele sistema.

Os testes de recuperação compararam 24 respostas UDP, com 8.940 bytes de payload.
Somadas às 84 do teste de integração, foram 108 respostas e 15.550 bytes conferidos.
Isso verifica correção funcional nos casos ensaiados, não throughput ou latência.

Os reinícios observados na VM ocorreram com zero mensagens pendentes no ponto da
troca. O comportamento de purga em filas ocupadas, circulares e com várias origens
foi verificado nos testes do mesmo módulo de controle usado pelo kernel.

## Artefatos verificados

| Arquivo | Bytes | SHA-256 |
|---|---:|---|
| `saida/raiz-os.img` | 134217728 | `e3d637664af0a65af430b0696db1f522540bd21a72a5edc178980f367779e60e` |
| `saida/esp/EFI/BOOT/BOOTX64.EFI` | 781824 | `b6d949e6fa4bd4f7cb007018818d3dcce5e567336eb26caeff356dbb45b81947` |

O construtor conferiu GPT primária/secundária, CRCs, FAT32 e os bytes EFI após
gravar a imagem. O arquivo EFI incorpora o núcleo e os três serviços.
Log: [compilacao-0.2.log](compilacao-0.2.log).

## Como reproduzir

Com Rust, QEMU e OVMF instalados, a partir da raiz do projeto:

```sh
cargo fmt --all --check
cargo test -p protocolos -p ferramentas -p contrato-boot -p contrato-sistema
cargo test -p nucleo --lib
cargo raiz compilar
cargo raiz testar
cargo raiz testar-isolamento
cargo raiz testar-recuperacao
```

O arquivo `rust-toolchain.toml` fixa a versão e os alvos de Rust. Configure
`RAIZ_QEMU`, `RAIZ_OVMF` e `RAIZ_OVMF_VARS` conforme o README quando necessário.
Os testes usam a imagem existente: recompile após alterar código do SO.

## Relatórios detalhados

- [Boot, rede e falha isolada](RELATORIO-SMOKE.md).
- [Memória e preempção](RELATORIO-ISOLAMENTO.md).
- [Supervisor e recuperação](RELATORIO-RECUPERACAO.md).
- [Avaliação pelos conceitos de Tanenbaum](../docs/CRITERIOS-TANENBAUM.md).
- [Decisão de ciclo de vida](../docs/ADR-002-CICLO-DE-VIDA.md).

Os logs serial e stderr de cada cenário estão no mesmo diretório. Os resultados
anteriores da versão 0.1 estão preservados em `v0.1/`.

## Limites

A recuperação cobre somente a rede: quatro encarnações por boot, com memória
antiga em quarentena e estado de protocolos reiniciado. O modo automático observa
falha/término; um loop exige pedido explícito. Terminal e supervisor não são
reiniciáveis nesta etapa.

Os resultados não demonstram IOMMU, proteção contra DMA malicioso, drenagem de
todas as operações de dispositivo, recuperação de conexões, persistência,
execução dinâmica de aplicações, suporte físico, garantia de tempo real ou
prontidão para produção. A imagem continua sendo um artefato de laboratório.
