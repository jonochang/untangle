package main

import (
	"fmt"
	"github.com/user/project/pkg/foo"
	"github.com/user/project/pkg/bar"
)

func main() {
	fmt.Println(foo.Hello())
	fmt.Println(bar.World())
}
