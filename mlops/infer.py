"""Hierarchical inference for Smart Coffee E-Nose.
Loads a trained model.pkl (3 independent SVM/LDA heads with StandardScaler),
extracts per-window mean/std features, and returns Level 1/2/3 predictions
with decision_function-based confidence scores.

Input  (stdin JSON):
  { "values": [[ch0,ch1,...,ch7], ...],          // raw time-series rows
    "feature_names": ["sensor_a","sensor_b",...], // must match training order
    "level_names": ["Spesies","Proses","Profil"]  // optional display labels
  }

Output (stdout JSON):
  { "ok": true,
    "levels": [
      {"level":1, "name":"Spesies",  "prediction":"Arabika", "confidence":0.934, "classes":["Arabika","Robusta"]},
      {"level":2, "name":"Proses",   "prediction":"Roasted", "confidence":0.881, "classes":["GreenBean","Roasted"]},
      {"level":3, "name":"Profil",   "prediction":"C03",     "confidence":0.745, "classes":["C01","C02",...]}
    ]
  }

On error: { "ok": false, "error": "message" }
"""
import json, pickle, sys
import numpy as np

LEVEL_DEFAULTS = ["Spesies", "Proses", "Profil Aroma"]

def extract_features(values):
    """Per-window mean + std, same as retrain.features() for a single sample."""
    arr = np.asarray(values, dtype=np.float64)
    # Convert mV to V if values are likely in mV scale (e.g. 0-4095 ADC values)
    if np.mean(arr) > 50.0: # Heuristic: if mean value is > 50, assume mV and scale to V
        arr = arr / 1000.0
    if arr.ndim != 2 or arr.shape[0] < 2 or arr.shape[1] < 1:
        raise ValueError(f"values must be 2D with >=2 rows, got shape {arr.shape}")
    if not np.all(np.isfinite(arr)):
        raise ValueError("values contain NaN or Inf")
    return np.concatenate([np.mean(arr, axis=0), np.std(arr, axis=0)]).reshape(1, -1)

def confidence_from_model(pipe, X):
    """Extract confidence score from SVM decision_function or LDA predict_proba."""
    estimator = pipe[-1]  # last step in Pipeline
    classes = estimator.classes_.tolist()

    # Try predict_proba first (LDA always has it; SVM only if probability=True)
    if hasattr(estimator, "predict_proba"):
        proba = pipe.predict_proba(X)[0]
        idx = int(np.argmax(proba))
        return classes[idx], float(proba[idx]), classes

    # Fallback: decision_function -> sigmoid approximation
    if hasattr(estimator, "decision_function"):
        df = pipe.decision_function(X)
        if df.ndim == 1:
            # Binary case: single value
            score = float(df[0])
            conf = 1.0 / (1.0 + np.exp(-abs(score)))
            idx = 1 if score > 0 else 0
            return classes[idx], float(conf), classes
        else:
            # Multi-class OvO/OvR
            idx = int(np.argmax(df[0]))
            # Softmax-like normalization
            exp_scores = np.exp(df[0] - np.max(df[0]))
            proba = exp_scores / exp_scores.sum()
            return classes[idx], float(proba[idx]), classes

    # Last resort: just predict, confidence unknown
    pred = pipe.predict(X)[0]
    return str(pred), 0.0, classes

def main():
    try:
        raw = sys.stdin.buffer.read()
        if len(raw) > 32 * 1024 * 1024:
            raise ValueError("Input exceeds 32 MiB")
        req = json.loads(raw)

        model_path = req.get("model_path")
        if not model_path:
            raise ValueError("model_path required")

        values = req.get("values")
        if not values or not isinstance(values, list):
            raise ValueError("values (2D array of sensor readings) required")

        level_names = req.get("level_names", LEVEL_DEFAULTS)

        # Load model bundle
        with open(model_path, "rb") as f:
            bundle = pickle.load(f)

        models = bundle["models"]  # list of 3 Pipelines
        expected_features = bundle.get("feature_names", [])

        # Adapt feature count gracefully to model's expected features
        n_channels = len(values[0]) if values else 0
        n_exp = len(expected_features) if expected_features else n_channels
        if n_exp > 0 and n_channels != n_exp:
            if n_channels > n_exp:
                values = [row[:n_exp] for row in values]
            else:
                values = [row + [0.0] * (n_exp - n_channels) for row in values]

        # Extract features
        X = extract_features(values)

        # Run 3-level hierarchical inference
        levels = []
        for i, model in enumerate(models):
            pred, conf, classes = confidence_from_model(model, X)
            levels.append({
                "level": i + 1,
                "name": level_names[i] if i < len(level_names) else f"Level {i+1}",
                "prediction": str(pred),
                "confidence": round(conf, 4),
                "classes": [str(c) for c in classes],
            })

        result = {"ok": True, "levels": levels}
        print(json.dumps(result, ensure_ascii=False))

    except Exception as e:
        print(json.dumps({"ok": False, "error": str(e)}, ensure_ascii=False))
        sys.exit(1)

if __name__ == "__main__":
    main()
