#!/bin/zsh
#echo "Creating binary..."
#arm-none-eabi-objcopy -O binary ./target/thumbv6m-none-eabi/release/vhrdbms-l010k8 ./target/thumbv6m-none-eabi/release/vhrdbms-l010k8.bin
echo "Resetting..."
/Applications/SEGGER/JLink_V662a/JLinkExe -CommanderScript ./reset.jlink
echo "Done."

