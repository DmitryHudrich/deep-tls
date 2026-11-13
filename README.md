# deep-tls

Демо: распознаём тип блокчейн-операции кошелька по зашифрованному TLS-трафику, видя только
длины, направление и тайминги TLS-записей (идея из Deep Fingerprinting, p-FP, ET-BERT).

Сеть не используется, всё эмулируется локально:

1. `node.rs`: мок тестового узла. Ethereum JSON-RPC (`eth_*`, включая batch) и Tron HTTP API (`/wallet/*`)
   отвечают JSON-ом в форме настоящих ответов. Есть состояние: транзакции какое-то время «в мемпуле»,
   поэтому кошелёк опрашивает квитанцию несколько раз.
2. `wallet.rs`: сценарии кошелька. Каждая операция — это последовательность вызовов узла:

   | класс | вызовы |
   |---|---|
   | `eth-send` | chainId, getTransactionCount, getBlockByNumber, maxPriorityFeePerGas, estimateGas, sendRawTransaction, опрос getTransactionReceipt |
   | `erc20-send` | то же + `eth_call balanceOf`, calldata `transfer`, лог Transfer в квитанции |
   | `eth-portf` | batch: getBalance + N × `eth_call balanceOf` |
   | `eth-logs` | blockNumber, getLogs по диапазону блоков |
   | `trx-send` | createtransaction, broadcasttransaction, опрос gettransactioninfobyid |
   | `trc20-send` | triggersmartcontract, broadcasttransaction, опрос gettransactioninfobyid |
   | `tron-acct` | getaccount, getaccountresource, triggerconstantcontract `balanceOf` |

   Случайно добавляются фоновые запросы (`eth_blockNumber` / `getnowblock`), которые мешают классификатору.
3. `net.rs`: HTTP-запросы и ответы шифруются в записи TLS 1.3 (AEAD, опционально с паддингом), режутся
   на TCP-сегменты по MSS, к ним добавляются RTT, пропускная способность и время обработки на сервере.
4. `tls.rs`: пассивный наблюдатель собирает TLS-записи из сегментов по каждому направлению, читая только заголовки.
5. `features.rs` + `knn.rs`: признаки (объём, число записей, всплески, длительность, первые 16 записей со знаком направления)
   и k-NN по z-нормированным признакам.

## Запуск

```bash
cargo run --release
```

```bash
cargo run --release -- --seed 7 --per-class 300 --k 3
```

```bash
cargo test
```

Без защиты точность около 97%: путаются только `eth-send` и `erc20-send`. Паддинг до 1 KiB почти не помогает.
Паддинг до 16 KiB (каждая запись максимального размера) снижает точность примерно до 64%, но классы всё равно
различимы по числу записей и таймингам: паддинг длин не скрывает «форму» сессии.
