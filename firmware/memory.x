/* Карта памяти nRF52833. Источник: nRF52833 PS v0.7 (4452_021)
 *   §4.2 Memory p.19 — 512kB flash, 128kB RAM
 *   §4.2.3 Memory map Fig.3 p.21 — Flash @0x00000000, Data RAM @0x20000000
 */

MEMORY
{
  FLASH : ORIGIN = 0x00000000, LENGTH = 512K
  RAM   : ORIGIN = 0x20000000, LENGTH = 128K
}
