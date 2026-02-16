package main

import (
	"fmt"
	"github.com/example/api/internal/db"
)

func main() {
	fmt.Println("api service")
	db.Connect()
}
