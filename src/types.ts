// Mirrors the Rust types returned by the `value_tablets` Tauri command.

export type Affix = "Prefix" | "Suffix" | "Unknown";
export type Confidence = "High" | "Medium" | "Low";

export interface ModValue {
  stat_hash: string;
  description: string;
  affix: Affix;
  per_unit_exalted: number;
  typical_roll: number;
  value_exalted: number;
  ci_low: number;
  ci_high: number;
  sample_size: number;
  confidence: Confidence;
}

export interface TypeValuation {
  tablet_type: string;
  listings_used: number;
  r2: number;
  base_value_exalted: number;
  listings_available: number | null;
  note: string | null;
  mods: ModValue[];
}

export interface HistoryPoint {
  at: string;
  value_exalted: number;
}

export interface Valuation {
  league: string;
  updated_at: string;
  divine_to_exalted: number | null;
  types: TypeValuation[];
}

/** A mod row flattened across types, carrying its tablet type + that type's fit. */
export interface ModRow extends ModValue {
  tablet_type: string;
  type_r2: number;
  type_note: string | null;
  type_supply: number | null;
}
