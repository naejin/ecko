// Expected: exit 1, check=unused-imports
package main

import (
	"fmt"
	"os"
)

func main() {
	x := 1
	_ = x
}
