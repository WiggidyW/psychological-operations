package main

import (
	"fmt"

	"github.com/ObjectiveAI/objectiveai/objectiveai-go"
)

func main() {
	client := objectiveai.NewClient()
	fmt.Println("psychological-operations initialized", client)
}
