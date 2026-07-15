package sqlite

import (
	"context"
	"database/sql"
	"errors"
	"fmt"
	"strings"
	"time"
)

type SentItem struct {
	Title  string    `json:"title"`
	URL    string    `json:"url"`
	SentAt time.Time `json:"sent_at"`
}

type Delivery struct {
	RunID       string
	Message     string
	DeliveredAt time.Time
}

func (s *Store) RecentDeliveries(ctx context.Context, limit int) ([]Delivery, error) {
	if limit <= 0 {
		limit = 2
	}
	rows, err := s.db.QueryContext(ctx, `
SELECT run_id, message, delivered_at FROM delivery
ORDER BY delivered_at DESC, id DESC
LIMIT ?`, limit)
	if err != nil {
		return nil, err
	}
	defer func() {
		_ = rows.Close()
	}()
	var deliveries []Delivery
	for rows.Next() {
		var delivery Delivery
		var deliveredAt string
		if err := rows.Scan(&delivery.RunID, &delivery.Message, &deliveredAt); err != nil {
			return nil, err
		}
		delivery.DeliveredAt, _ = time.Parse(time.RFC3339Nano, deliveredAt)
		deliveries = append(deliveries, delivery)
	}
	return deliveries, rows.Err()
}

func (s *Store) RecentSentItems(ctx context.Context, since time.Time) ([]SentItem, error) {
	rows, err := s.db.QueryContext(ctx, `
SELECT title, url, sent_at FROM sent_item
WHERE sent_at >= ?
ORDER BY sent_at DESC, id DESC`, since.UTC().Format(time.RFC3339Nano))
	if err != nil {
		return nil, err
	}
	defer func() {
		_ = rows.Close()
	}()
	var items []SentItem
	for rows.Next() {
		var item SentItem
		var sentAt string
		if err := rows.Scan(&item.Title, &item.URL, &sentAt); err != nil {
			return nil, err
		}
		item.SentAt, _ = time.Parse(time.RFC3339Nano, sentAt)
		items = append(items, item)
	}
	return items, rows.Err()
}

func (s *Store) InsertDelivery(ctx context.Context, runID string, message string, items []SentItem) ([]SentItem, error) {
	if strings.TrimSpace(runID) == "" {
		return nil, errors.New("run_id is required")
	}
	tx, err := s.db.BeginTx(ctx, nil)
	if err != nil {
		return nil, err
	}
	claim, err := tx.ExecContext(ctx, `
INSERT INTO delivery_once (run_id, message)
VALUES (?, ?)
ON CONFLICT(run_id) DO NOTHING`, runID, message)
	if err != nil {
		_ = tx.Rollback()
		return nil, err
	}
	claimed, err := claim.RowsAffected()
	if err != nil {
		_ = tx.Rollback()
		return nil, err
	}
	if claimed == 0 {
		var storedMessage string
		var deliveryID sql.NullInt64
		if err := tx.QueryRowContext(ctx, `SELECT message, delivery_id FROM delivery_once WHERE run_id = ?`, runID).Scan(&storedMessage, &deliveryID); err != nil {
			_ = tx.Rollback()
			return nil, err
		}
		if storedMessage != message {
			_ = tx.Rollback()
			return nil, errors.New("run_id was already delivered with a different message")
		}
		if !deliveryID.Valid {
			_ = tx.Rollback()
			return nil, errors.New("delivery idempotency record is incomplete")
		}
		stored, err := sentItemsForDelivery(ctx, tx, deliveryID.Int64)
		if err != nil {
			_ = tx.Rollback()
			return nil, err
		}
		if err := tx.Commit(); err != nil {
			return nil, err
		}
		return stored, nil
	}
	deliveredAt := s.now().UTC()
	result, err := tx.ExecContext(ctx, `
INSERT INTO delivery (run_id, message, delivered_at)
VALUES (?, ?, ?)`, runID, message, deliveredAt.Format(time.RFC3339Nano))
	if err != nil {
		_ = tx.Rollback()
		return nil, err
	}
	deliveryID, err := result.LastInsertId()
	if err != nil {
		_ = tx.Rollback()
		return nil, err
	}
	for i := range items {
		items[i].SentAt = deliveredAt
		if _, err := tx.ExecContext(ctx, `
INSERT INTO sent_item (delivery_id, run_id, title, url, title_key, sent_at)
VALUES (?, ?, ?, ?, ?, ?)`,
			deliveryID, runID, items[i].Title, items[i].URL, NormalizeTitleKey(items[i].Title), deliveredAt.Format(time.RFC3339Nano)); err != nil {
			_ = tx.Rollback()
			return nil, err
		}
	}
	if _, err := tx.ExecContext(ctx, `UPDATE delivery_once SET delivery_id = ? WHERE run_id = ?`, deliveryID, runID); err != nil {
		_ = tx.Rollback()
		return nil, err
	}
	if err := tx.Commit(); err != nil {
		return nil, err
	}
	return items, nil
}

func sentItemsForDelivery(ctx context.Context, tx *sql.Tx, deliveryID int64) ([]SentItem, error) {
	rows, err := tx.QueryContext(ctx, `SELECT title, url, sent_at FROM sent_item WHERE delivery_id = ? ORDER BY id`, deliveryID)
	if err != nil {
		return nil, err
	}
	defer func() { _ = rows.Close() }()
	var items []SentItem
	for rows.Next() {
		var item SentItem
		var sentAt string
		if err := rows.Scan(&item.Title, &item.URL, &sentAt); err != nil {
			return nil, err
		}
		item.SentAt, _ = time.Parse(time.RFC3339Nano, sentAt)
		items = append(items, item)
	}
	if err := rows.Err(); err != nil {
		return nil, fmt.Errorf("read idempotent delivery items: %w", err)
	}
	return items, nil
}

func NormalizeTitleKey(text string) string {
	text = strings.ToLower(strings.TrimSpace(text))
	var b strings.Builder
	previousSpace := false
	for _, r := range text {
		switch {
		case r >= 'a' && r <= 'z', r >= '0' && r <= '9':
			b.WriteRune(r)
			previousSpace = false
		default:
			if !previousSpace {
				b.WriteByte(' ')
				previousSpace = true
			}
		}
	}
	return strings.Join(strings.Fields(b.String()), " ")
}
