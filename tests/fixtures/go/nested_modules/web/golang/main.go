package main

import (
	"fmt"
	"github.com/example/web/pkg/handler"
)

func main() {
	fmt.Println("web service")
	handler.Handle()
}
