# BidAsk 欄位整理

## 整體架構邏輯

| 項目 | 說明 |
|---|---|
| solace pub topic | `IS/V1/QUO/(exchange)/(code)>` |
| 24=1 | 代表 refresh，排除掉該筆 |
| 20 | 有值為一筆 QUO |

## 欄位明細

| 欄位 | 欄位名稱 | type | token | 邏輯 | 源頭邏輯註記 |
|---|---|---|---|---|---|
| 商品代碼 | code | str | 5 |  |  |
| CSP時間 | datetime | str | 16 |  | 16&20源頭給相同 |
| 交易所時間 | datetime | str | 55 |  |  |
| 委買價 | bid_price | [str(decimal)] | 12 |  |  |
| 委買量 | bid_volume | [uint] | 13 |  |  |
| 委賣價 | ask_price | [str(decimal)] | 10 |  |  |
| 委賣量 | ask_volume | [uint] | 11 |  |  |
| 盤別 | market_phase | u8 | 1709 | 開收盤會傳送過來一次 |  |
| 暫停交易 | tradable_status | u8 | 1708 |  |  |
| 流水號 | SerialNum | uint |  | 自編作為識別 |  |
