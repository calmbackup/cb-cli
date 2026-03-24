package cmd

import (
	"fmt"
	"math/rand"

	"github.com/charmbracelet/lipgloss"
)

var (
	brandStyle = lipgloss.NewStyle().Bold(true)
	dimStyle   = lipgloss.NewStyle().Faint(true)
)

// brandHeader returns the branded header line: 🪷 Calm Backup
func brandHeader() string {
	return fmt.Sprintf("🪷 %s", brandStyle.Render("Calm Backup"))
}

// brandSignature returns a random reassuring sign-off message.
func brandSignature() string {
	messages := []string{
		"Your data is safe with us.",
		"Encrypted. Automated. Calm.",
		"Sleep well, your backups are handled.",
		"Zero-knowledge. Zero worry.",
		"One less thing to think about.",
		"Backed up and encrypted, as always.",
		"Your backups are in good hands.",
		"Relax. We've got your data covered.",
		"Secure backups, peaceful mind.",
		"Protection that runs itself.",
	}
	msg := messages[rand.Intn(len(messages))]
	return dimStyle.Render(fmt.Sprintf("  %s", msg))
}
