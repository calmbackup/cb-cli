package cmd

import (
	"fmt"
	"math/rand"

	"github.com/charmbracelet/lipgloss"
)

var (
	brandStyle   = lipgloss.NewStyle().Bold(true)
	dimStyle     = lipgloss.NewStyle().Faint(true)
	stepStyle    = lipgloss.NewStyle().Foreground(lipgloss.Color("6"))
	successStyle = lipgloss.NewStyle().Foreground(lipgloss.Color("2")).Bold(true)
	labelStyle   = lipgloss.NewStyle().Faint(true)
	headerStyle  = lipgloss.NewStyle().Bold(true).Underline(true)
)

const pad = "  "

// brandHeader returns the branded header block with top padding and tagline.
func brandHeader() string {
	brand := fmt.Sprintf("%s🪷 %s", pad, brandStyle.Render("Calm Backup"))
	tagline := dimStyle.Render(fmt.Sprintf("%s%s", pad, randomTagline()))
	return fmt.Sprintf("\n\n%s\n%s\n\n", brand, tagline)
}

// brandFooter returns the closing signature.
func brandFooter() string {
	return ""
}

// printStep prints a progress step with an arrow indicator.
func printStep(msg string) {
	fmt.Printf("%s%s %s\n", pad, stepStyle.Render("→"), msg)
}

// printDone prints a success step with a checkmark.
func printDone(msg string) {
	fmt.Printf("%s%s %s\n", pad, successStyle.Render("✓"), msg)
}

// printInfo prints an informational line with padding.
func printInfo(msg string) {
	fmt.Printf("%s%s\n", pad, msg)
}

// printLabel prints a key-value pair with styled label.
func printLabel(label, value string) {
	fmt.Printf("%s%-16s %s\n", pad, labelStyle.Render(label), value)
}

// printSection prints a section header.
func printSection(title string) {
	fmt.Printf("\n%s%s\n", pad, headerStyle.Render(title))
}

// printSuccess prints a prominent success banner with spacing.
func printSuccess(msg string) {
	style := lipgloss.NewStyle().Bold(true).Foreground(lipgloss.Color("2"))
	fmt.Printf("\n\n%s🪷 %s\n\n", pad, style.Render(msg))
}

func randomTagline() string {
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
	return messages[rand.Intn(len(messages))]
}
