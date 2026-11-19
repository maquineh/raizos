# RZ-024 — conclusão no recorte testado

RZ-024 está **concluído no escopo testado**. Três subcasos opt-in QEMU foram
aprovados em [teste-rz024-1794742020545159600](../teste-rz024-1794742020545159600/resultado.txt).
Os seriais e registros de ambiente por subcaso estão ligados abaixo. Os números
são contagens de interrupções observadas em uma janela de cada fixture; não são
durações de parede, medidas de energia, metas de desempenho ou evidência de
suporte a hardware físico.

| Subcaso | Resultado e serial | Observação registrada |
| --- | --- | --- |
| IRQ4 em idle | [resultado](../teste-rz024-1794742020545159600/irq4/resultado.txt), [serial](../teste-rz024-1794742020545159600/irq4/serial.log), [ambiente](../teste-rz024-1794742020545159600/irq4/ambiente.txt) | Uma entrada em idle; 5 IRQ0 em idle, 1 IRQ4; terminal selecionado. |
| Prazo em idle | [resultado](../teste-rz024-1794742020545159600/prazo/resultado.txt), [serial](../teste-rz024-1794742020545159600/prazo/serial.log), [ambiente](../teste-rz024-1794742020545159600/prazo/ambiente.txt) | Uma entrada em idle; 199 IRQ0 em idle, 0 IRQ4; supervisor selecionado, timeout e resposta registrados. |
| Pressão finita | [resultado](../teste-rz024-1794742020545159600/pressao-finita/resultado.txt), [serial](../teste-rz024-1794742020545159600/pressao-finita/serial.log), [ambiente](../teste-rz024-1794742020545159600/pressao-finita/ambiente.txt) | Fixture finita registrou 8 IPCs ordenadas e eco completo; não estabelece meta de desempenho. |

Cada cenário tem imagem, QEMU/firmware e configuração identificados em seu
`ambiente.txt`; os `qemu-stderr.log` dos três subcasos são arquivos atuais
preservados junto aos seriais.

## Validação normal e reprodutibilidade

A validação normal em [execucao-rz-024-20261115](.) registrou cinco boots
(smoke, isolamento e recuperação), `cargo fmt --all --check`, 37 testes de
pacotes (3 + 25 + 9) e 34 testes do núcleo, além de build e verificação da
imagem. Os logs de comando e relatórios estão nesta pasta. Os manifests de
[fontes](fontes.sha256) e [artefatos](artefatos.sha256) identificam essa
validação.

Os `qemu-*-stderr.log` copiados junto aos relatórios dos boots normais têm data
histórica de 09/11/2026; não os atribua a esta execução. Os seriais, relatórios
e logs de comando atuais permanecem preservados.

## Tentativa anterior

A tentativa [teste-rz024-1794741842151523900](../teste-rz024-1794741842151523900/resultado.txt)
([subcaso IRQ4](../teste-rz024-1794741842151523900/irq4/resultado.txt)) falhou
porque o parser recebeu um prompt serial sem newline e encerrou com
`Serial terminou com linha incompleta`. O log do comando está em
[comando-rz024-1.log](comando-rz024-1.log); a execução posterior usou o parser
corrigido, preservou a tentativa anterior e aprovou os três subcasos. Nenhum
resultado histórico foi sobrescrito.

## Limites

As amostras IRQ0 são finitas e observadas nas janelas impressas nos seriais. Não
demonstram ausência de trabalho ou IRQ fora dessas janelas, consumo de energia,
latência/throughput, metas percentuais, operação contínua, hardware físico ou
eliminação do polling periódico normal da rede e do PS/2. RZ-024 fecha apenas o
critério controlado deste item; a matriz T03 e o marco 0.3 continuam abertos.
