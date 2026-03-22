// Expected: exit 0
// Blank imports are side-effect imports (e.g., driver registration).
package main

import (
	"database/sql"
	_ "github.com/lib/pq"
)

func main() {
	db, err := sql.Open("postgres", "host=localhost")
	if err != nil {
		panic(err)
	}
	defer db.Close()
}
