// Expected: exit 0
package main

import (
	"encoding/json"
	"fmt"
	"os"
)

type Config struct {
	Name string `json:"name"`
	Port int    `json:"port"`
}

func LoadConfig(path string) (*Config, error) {
	data, err := os.ReadFile(path)
	if err != nil {
		return nil, fmt.Errorf("read config: %w", err)
	}
	var cfg Config
	if err := json.Unmarshal(data, &cfg); err != nil {
		return nil, fmt.Errorf("parse config: %w", err)
	}
	return &cfg, nil
}

func main() {
	cfg, err := LoadConfig("config.json")
	if err != nil {
		fmt.Fprintf(os.Stderr, "error: %v\n", err)
		os.Exit(1)
	}
	fmt.Println(cfg.Name, cfg.Port)
}
