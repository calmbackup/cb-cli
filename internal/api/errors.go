package api

import (
	"encoding/json"
	"errors"
	"fmt"
)

var (
	ErrAuthentication = errors.New("authentication failed")
	ErrBilling        = errors.New("billing issue")
	ErrBackupDeleted  = errors.New("backup was previously deleted")
	ErrSizeLimit      = errors.New("size limit exceeded")
	ErrValidation     = errors.New("validation error")
	ErrRateLimit      = errors.New("rate limit exceeded")
	ErrServer         = errors.New("server error")
)

type APIError struct {
	StatusCode int
	Message    string
	Details    map[string]interface{}
}

func (e *APIError) Error() string {
	if e.Message != "" {
		return fmt.Sprintf("API error %d: %s", e.StatusCode, e.Message)
	}

	return fmt.Sprintf("API error %d", e.StatusCode)
}

func (e *APIError) Is(target error) bool {
	switch {
	case target == ErrAuthentication:
		return e.StatusCode == 401
	case target == ErrBilling:
		return e.StatusCode == 402
	case target == ErrBackupDeleted:
		return e.StatusCode == 409
	case target == ErrSizeLimit:
		return e.StatusCode == 413
	case target == ErrValidation:
		return e.StatusCode == 422
	case target == ErrRateLimit:
		return e.StatusCode == 429
	case target == ErrServer:
		return e.StatusCode >= 500
	default:
		return false
	}
}

func parseAPIError(statusCode int, body []byte) *APIError {
	apiErr := &APIError{StatusCode: statusCode}

	var parsed struct {
		Message string                 `json:"message"`
		Details map[string]interface{} `json:"details"`
	}

	if err := json.Unmarshal(body, &parsed); err == nil {
		apiErr.Message = parsed.Message
		apiErr.Details = parsed.Details
	}

	return apiErr
}
