// Expected: exit 0
// Aliased imports: usage detection must use the alias, not the path segment.
package main

import (
	j "encoding/json"
	"fmt"
)

func main() {
	data, _ := j.Marshal(map[string]int{"a": 1})
	fmt.Println(string(data))
}
