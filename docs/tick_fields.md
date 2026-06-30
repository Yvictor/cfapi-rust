# Tick 欄位整理

## 整體架構邏輯

| 項目 | 說明 |
|---|---|
| solace pub topic | `IS/V1/TIC/(exchange)/(code)>` |
| 1021 | 有值為一筆 TIC |
| 24=1 | 代表 refresh，可排除掉該筆 |

## 欄位明細

| 欄位 | 欄位名稱 | type | token | 處理邏輯 | 源頭邏輯註記 |
|---|---|---|---|---|---|
| 商品代碼 | code | str | 5 |  |  |
| CSP時間 | datetime | str | 16 |  | 16&18源頭給相同 |
| 交易所時間 | datetime | str | 55 |  |  |
| 開盤價 | open | str(decimal) | 400 | 第一筆會傳送過來一次 | 400整股才給 |
| 均價 | avg_price | str(decimal) | 474 |  | 474整股才給 |
| 成交價 | close | str(decimal) | 8/447 | 8,447二擇一。8為整股、447為碎股 |  |
| 最高價 | high | str(decimal) | 388 |  |  |
| 最低價 | low | str(decimal) | 394 |  |  |
| 成交額 | amount | uint |  | 成交量*成交價 |  |
| 總成交額 | total_amount | str(decimal) | 460 |  |  |
| 成交量 | volume | uint | 9/448 | 9,448二擇一。9為整股、448為碎股 |  |
| 總成交量 | total_volume | uint | 463 |  |  |
| 內外盤別 | tick_type | u8 |  | 成交價與token10/12比較 |  |
| 漲跌註記 | chg_type | u8 | 316 | 整股直接使用316、碎股與開盤價比較 | 316整股才給 |
| 漲跌 | price_chg | str(decimal) | 361 | 整股直接使用361、碎股與開盤價比較 | 361整股才給 |
| 漲跌幅 | pct_chg | str(decimal) | 362 | 整股直接使用362、碎股與開盤價比較 | 362整股才給 |
| 買盤成交總量 | bid_side_total_vol | uint |  | 依成交量和內外盤別自行計算 |  |
| 賣盤成交總量 | ask_side_total_vol | uint |  | 依成交量和內外盤別自行計算 |  |
| 買盤成交筆數 | bid_side_total_cnt | uint |  | 依內外盤別自行累加 |  |
| 賣盤成交筆數 | ask_side_total_cnt | uint |  | 依內外盤別自行累加 |  |
| 盤別 | market_phase | u8 | 1709 | 開收盤會傳送過來一次 |  |
| 暫停交易 | tradable_status | u8 | 1708 |  |  |
| 交易類型 | trade_cond | uint | 2500 |  |  |
| 流水號 | SerialNum | uint |  | 自編作為識別 |  |
