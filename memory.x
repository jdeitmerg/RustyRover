MEMORY
{
    /* Requirements for SoftDevice S112 v7.3.0:
     * 100kB (0x19000 bytes) of flash
     * at least 3.7kB (0xEB8 bytes) of RAM, actual value determined during
     * runtime test.
     */
    FLASH : ORIGIN = 0x0000000 + 0x19000, LENGTH = 256K - 0x19000
    RAM : ORIGIN = 0x20000000 + 0x1AE0, LENGTH = 32K - 0x1AE0
}
