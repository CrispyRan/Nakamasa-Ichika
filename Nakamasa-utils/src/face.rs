//! 人脸识别模块 — 基于 ONNX Runtime 的本地人脸检测与识别
//!
//! # 模型
//!
//! - **检测**: YuNet ([OpenCV Zoo](https://github.com/opencv/opencv_zoo)) — ~300KB
//! - **识别**: ArcFace ([insightface](https://github.com/deepinsight/insightface)) — 512 维嵌入
//!
//! # 使用
//!
//! ```rust,ignore
//! use nakamasa_utils::face::FaceEngine;
//!
//! let engine = FaceEngine::new("yunet.onnx", "arcface.onnx")?;
//! let results = engine.recognize(b"image_data...")?;
//! for r in &results {
//!     println!("face at ({}, {}), conf={}", r.face.x, r.face.y, r.face.confidence);
//! }
//! let sim = FaceEngine::cosine_similarity(&emb1, &emb2);
//! ```

use std::path::Path;

use image::DynamicImage;
use ndarray::Array4;
use ort::session::Session;
use thiserror::Error;

// ============================================================================
// 错误类型
// ============================================================================

/// 人脸识别错误
#[derive(Error, Debug)]
pub enum FaceError {
    #[error("ONNX Runtime 错误: {0}")]
    Ort(String),
    #[error("图像处理错误: {0}")]
    Image(String),
    #[error("模型错误: {0}")]
    Model(String),
    #[error("未检测到人脸")]
    NoFace,
    #[error("参数错误: {0}")]
    InvalidArgument(String),
}

impl From<image::ImageError> for FaceError {
    fn from(e: image::ImageError) -> Self {
        FaceError::Image(e.to_string())
    }
}

// ============================================================================
// 数据类型
// ============================================================================

/// 检测到的人脸
#[derive(Debug, Clone)]
pub struct Face {
    /// 人脸边界框 (x, y, w, h)，原始图像像素坐标
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    /// 检测置信度 [0, 1]
    pub confidence: f32,
    /// 5 个关键点 (x0,y0, x1,y1, ..., x4,y4)，原始图像像素坐标
    pub landmarks: Option<[f32; 10]>,
}

/// 识别结果
#[derive(Debug, Clone)]
pub struct RecognitionResult {
    pub face: Face,
    /// 512 维 L2 归一化人脸嵌入向量
    pub embedding: Vec<f32>,
}

// ============================================================================
// 人脸检测器
// ============================================================================

/// 基于 YuNet 的轻量人脸检测器
pub struct FaceDetector {
    pub session: Session,
    input_size: (u32, u32), // (w, h)
    input_name: String,
}

impl FaceDetector {
    /// 加载 YuNet ONNX 模型
    pub fn new(model_path: impl AsRef<Path>) -> Result<Self, FaceError> {
        let session = Session::builder()
            .map_err(|e| FaceError::Ort(e.to_string()))?
            .commit_from_file(model_path.as_ref())
            .map_err(|e| FaceError::Ort(e.to_string()))?;

        let input_name = session.inputs().iter().next()
            .map(|i| i.name().to_string())
            .unwrap_or_else(|| "input".into());

        let input_size = session.inputs().iter().next()
            .and_then(|i| i.dtype().tensor_shape())
            .and_then(|shape| {
                if shape.len() >= 4 {
                    Some((shape[3] as u32, shape[2] as u32))
                } else {
                    None
                }
            })
            .unwrap_or((640, 640));

        Ok(Self { session, input_size, input_name })
    }

    /// 检测图片中的人脸
    pub fn detect(&mut self, img: &DynamicImage) -> Result<Vec<Face>, FaceError> {
        let (iw, ih) = (img.width(), img.height());
        if iw == 0 || ih == 0 {
            return Err(FaceError::InvalidArgument("empty image".into()));
        }

        // 1. 缩放到模型输入尺寸
        let resized = img.resize_exact(
            self.input_size.0,
            self.input_size.1,
            image::imageops::FilterType::Triangle,
        );
        let rgb = resized.to_rgb8();

        // 2. HWC → CHW + 归一化 [0, 255] → [0, 1]
        let (w, h) = (self.input_size.0, self.input_size.1);
        let n = (w * h) as usize;
        let mut input_data = vec![0.0f32; n * 3];
        for y in 0..h {
            for x in 0..w {
                let px = rgb.get_pixel(x, y);
                let idx = (y * w + x) as usize;
                input_data[idx] = px[0] as f32 / 255.0;
                input_data[n + idx] = px[1] as f32 / 255.0;
                input_data[n * 2 + idx] = px[2] as f32 / 255.0;
            }
        }

        // 3. 创建 NDArray 张量 (NCHW)
        let tensor = Array4::from_shape_vec(
            (1, 3, h as usize, w as usize),
            input_data,
        ).map_err(|e| FaceError::Ort(e.to_string()))?;

        // 4. 推理
        let outputs = self.session
            .run(ort::inputs![self.input_name.as_str() => ort::value::TensorRef::from_array_view(&tensor)
                .map_err(|e| FaceError::Ort(e.to_string()))?])
            .map_err(|e| FaceError::Ort(e.to_string()))?;

        // 5. 解析 YuNet 2023mar 输出
        // 12 个输出，3 个尺度 (stride=8, 16, 32):
        //   [0,1,2]:  scores  [1, N, 1]
        //   [3,4,5]:  extra   [1, N, 1] (忽略)
        //   [6,7,8]:  bboxes  [1, N, 4]  (dx, dy, dw, dh)
        //   [9,10,11]: kpts    [1, N, 10] (5 个关键点偏移)
        let output = &outputs[0];
        let (shape, _) = output.try_extract_tensor::<f32>()
            .map_err(|e| FaceError::Ort(e.to_string()))?;
        if shape.is_empty() {
            return Ok(Vec::new());
        }

        let mut faces = Vec::new();
        let strides = [8usize, 16, 32];
        for scale_idx in 0..3 {
            let scores_out = &outputs[scale_idx];
            let bboxes_out = &outputs[6 + scale_idx];
            let kpts_out = &outputs[9 + scale_idx];

            let (_, scores) = scores_out.try_extract_tensor::<f32>()
                .map_err(|e| FaceError::Ort(e.to_string()))?;
            let (_, bboxes) = bboxes_out.try_extract_tensor::<f32>()
                .map_err(|e| FaceError::Ort(e.to_string()))?;
            let (_, _kpts) = kpts_out.try_extract_tensor::<f32>()
                .map_err(|e| FaceError::Ort(e.to_string()))?;

            let stride = strides[scale_idx];
            let cols = self.input_size.0 as usize / stride;
            let num_det = scores.len();

            let scale_x = iw as f32 / self.input_size.0 as f32;
            let scale_y = ih as f32 / self.input_size.1 as f32;

            for i in 0..num_det {
                let conf = scores[i];
                if conf < 0.9 {
                    continue;
                }

                let bbox_base = i * 4;
                let _kpts_base = i * 10;

                if bbox_base + 3 >= bboxes.len() {
                    break;
                }

                // 解码 anchor-based bbox
                let grid_col = (i % cols) as f32;
                let grid_row = (i / cols) as f32;
                let s = stride as f32;

                let x_center = (grid_col + bboxes[bbox_base]) * s;
                let y_center = (grid_row + bboxes[bbox_base + 1]) * s;
                let bw = bboxes[bbox_base + 2].exp() * s;
                let bh = bboxes[bbox_base + 3].exp() * s;

                let x1 = (x_center - bw / 2.0).max(0.0);
                let y1 = (y_center - bh / 2.0).max(0.0);
                let x2 = (x_center + bw / 2.0).min(self.input_size.0 as f32);
                let y2 = (y_center + bh / 2.0).min(self.input_size.1 as f32);

                if x1 >= x2 || y1 >= y2 {
                    continue;
                }

                faces.push(Face {
                    x: x1 * scale_x,
                    y: y1 * scale_y,
                    w: (x2 - x1) * scale_x,
                    h: (y2 - y1) * scale_y,
                    confidence: conf,
                    landmarks: None,
                });
            }
        }

        // NMS (Non-Maximum Suppression)
        if faces.len() > 1 {
            // 按置信度降序排序
            faces.sort_by(|a, b| b.confidence.partial_cmp(&a.confidence).unwrap_or(std::cmp::Ordering::Equal));
            let mut keep = vec![true; faces.len()];
            for i in 0..faces.len() {
                if !keep[i] { continue; }
                let fi = &faces[i];
                let ax1 = fi.x; let ay1 = fi.y; let ax2 = fi.x + fi.w; let ay2 = fi.y + fi.h;
                let ai = (ax2 - ax1) * (ay2 - ay1);
                for j in (i + 1)..faces.len() {
                    if !keep[j] { continue; }
                    let fj = &faces[j];
                    let bx1 = fj.x; let by1 = fj.y; let bx2 = fj.x + fj.w; let by2 = fj.y + fj.h;
                    let ix1 = ax1.max(bx1); let iy1 = ay1.max(by1);
                    let ix2 = ax2.min(bx2); let iy2 = ay2.min(by2);
                    if ix1 < ix2 && iy1 < iy2 {
                        let inter = (ix2 - ix1) * (iy2 - iy1);
                        let bj = (bx2 - bx1) * (by2 - by1);
                        let iou = inter / (ai + bj - inter);
                        if iou > 0.08 {
                            keep[j] = false;
                        }
                    }
                }
            }
            faces = faces.into_iter().zip(keep.into_iter())
                .filter(|(_, k)| *k)
                .map(|(f, _)| f)
                .collect();
        }

        Ok(faces)
    }
}

// ============================================================================
// 人脸识别器
// ============================================================================

/// 基于 ArcFace/MobileFaceNet 的人脸嵌入提取器
pub struct FaceRecognizer {
    session: Session,
    input_size: (u32, u32), // (w, h)
    input_name: String,
}

impl FaceRecognizer {
    /// 加载 ArcFace ONNX 模型（输入 112x112 RGB，输出 512-dim 嵌入）
    pub fn new(model_path: impl AsRef<Path>) -> Result<Self, FaceError> {
        let session = Session::builder()
            .map_err(|e| FaceError::Ort(e.to_string()))?
            .commit_from_file(model_path.as_ref())
            .map_err(|e| FaceError::Ort(e.to_string()))?;

        let input_name = session.inputs().iter().next()
            .map(|i| i.name().to_string())
            .unwrap_or_else(|| "input".into());

        let input_size = session.inputs().iter().next()
            .and_then(|i| i.dtype().tensor_shape())
            .and_then(|shape| {
                if shape.len() >= 4 {
                    Some((shape[3] as u32, shape[2] as u32))
                } else {
                    None
                }
            })
            .unwrap_or((112, 112));

        Ok(Self { session, input_size, input_name })
    }

    /// 提取人脸区域的嵌入向量
    pub fn extract_embedding(&mut self, img: &DynamicImage, face: &Face) -> Result<Vec<f32>, FaceError> {
        // 1. 裁剪人脸区域（扩大 1.4 倍以包含更多上下文）
        let (iw, ih) = (img.width() as f32, img.height() as f32);
        let cx = face.x + face.w / 2.0;
        let cy = face.y + face.h / 2.0;
        let size = face.w.max(face.h) * 1.4;
        let x = (cx - size / 2.0).max(0.0).min(iw - 1.0);
        let y = (cy - size / 2.0).max(0.0).min(ih - 1.0);
        let crop_w = size.min(iw - x);
        let crop_h = size.min(ih - y);

        let face_img = if crop_w > 0.0 && crop_h > 0.0 {
            img.crop_imm(x as u32, y as u32, crop_w as u32, crop_h as u32)
        } else {
            return Err(FaceError::InvalidArgument("face crop out of bounds".into()));
        };

        // 2. 缩放到模型输入尺寸
        let resized = face_img.resize_exact(
            self.input_size.0,
            self.input_size.1,
            image::imageops::FilterType::Triangle,
        );
        let rgb = resized.to_rgb8();

        // 3. HWC → CHW + ArcFace 标准化: (pixel - 127.5) / 128.0 → [-1, 1]
        let (w, h) = (self.input_size.0, self.input_size.1);
        let n = (w * h) as usize;
        let mut input_data = vec![0.0f32; n * 3];
        for y in 0..h {
            for x in 0..w {
                let px = rgb.get_pixel(x, y);
                let idx = (y * w + x) as usize;
                input_data[idx] = (px[0] as f32 - 127.5) / 128.0;
                input_data[n + idx] = (px[1] as f32 - 127.5) / 128.0;
                input_data[n * 2 + idx] = (px[2] as f32 - 127.5) / 128.0;
            }
        }

        // 4. 创建张量
        let tensor = Array4::from_shape_vec(
            (1, 3, h as usize, w as usize),
            input_data,
        ).map_err(|e| FaceError::Ort(e.to_string()))?;

        // 5. 推理
        let outputs = self.session
            .run(ort::inputs![self.input_name.as_str() => ort::value::TensorRef::from_array_view(&tensor)
                .map_err(|e| FaceError::Ort(e.to_string()))?])
            .map_err(|e| FaceError::Ort(e.to_string()))?;

        // 6. 提取嵌入向量
        let output = &outputs[0];
        let (_shape, data) = output.try_extract_tensor::<f32>()
            .map_err(|e| FaceError::Ort(e.to_string()))?;

        if data.is_empty() {
            return Err(FaceError::Model("empty embedding output".into()));
        }

        // L2 归一化
        let norm: f32 = data.iter().map(|v| v * v).sum::<f32>().sqrt();
        let embedding: Vec<f32> = if norm > 0.0 {
            data.iter().map(|v| v / norm).collect()
        } else {
            data.to_vec()
        };

        Ok(embedding)
    }
}

// ============================================================================
// 人脸识别引擎（检测 + 识别组合）
// ============================================================================

/// 组合人脸检测与识别引擎
pub struct FaceEngine {
    detector: FaceDetector,
    recognizer: FaceRecognizer,
}

impl FaceEngine {
    /// 加载检测和识别模型
    ///
    /// - `detector_path` — YuNet ONNX 模型路径
    /// - `recognizer_path` — ArcFace ONNX 模型路径
    pub fn new(
        detector_path: impl AsRef<Path>,
        recognizer_path: impl AsRef<Path>,
    ) -> Result<Self, FaceError> {
        Ok(Self {
            detector: FaceDetector::new(detector_path)?,
            recognizer: FaceRecognizer::new(recognizer_path)?,
        })
    }

    /// 检测并识别图片中所有人脸
    ///
    /// `image_bytes` — JPEG/PNG 图片原始字节
    pub fn recognize(&mut self, image_bytes: &[u8]) -> Result<Vec<RecognitionResult>, FaceError> {
        let img = image::load_from_memory(image_bytes)?;
        let faces = self.detector.detect(&img)?;
        if faces.is_empty() {
            return Err(FaceError::NoFace);
        }
        let mut results = Vec::with_capacity(faces.len());
        for face in &faces {
            let embedding = self.recognizer.extract_embedding(&img, face)?;
            results.push(RecognitionResult {
                face: face.clone(),
                embedding,
            });
        }
        Ok(results)
    }

    /// 计算两个 L2 归一化嵌入向量的余弦相似度
    ///
    /// 返回值范围 [-1, 1]，通常 > 0.5 认为是同一人
    #[inline]
    pub fn cosine_similarity(emb1: &[f32], emb2: &[f32]) -> f32 {
        if emb1.len() != emb2.len() || emb1.is_empty() {
            return 0.0;
        }
        emb1.iter().zip(emb2.iter()).map(|(a, b)| a * b).sum()
    }

    /// 计算欧几里得距离
    #[inline]
    pub fn euclidean_distance(emb1: &[f32], emb2: &[f32]) -> f32 {
        if emb1.len() != emb2.len() || emb1.is_empty() {
            return f32::MAX;
        }
        emb1.iter()
            .zip(emb2.iter())
            .map(|(a, b)| (a - b) * (a - b))
            .sum::<f32>()
            .sqrt()
    }
}

// ============================================================================
// 模型下载工具
// ============================================================================

/// 默认模型下载地址
pub mod models {
    /// YuNet 人脸检测模型（~300KB）
    pub const YUNET_URL: &str =
        "https://github.com/opencv/opencv_zoo/raw/main/models/face_detection_yunet/face_detection_yunet_2023mar.onnx";

    /// ArcFace 人脸识别模型（~10MB）
    pub const ARCFACE_URL: &str =
        "https://github.com/onnx/models/raw/main/vision/body_analysis/arcface/model/arcfaceresnet100-8.onnx";

    /// 下载模型文件
    ///
    /// ```rust,ignore
    /// models::download("yunet.onnx", models::YUNET_URL).await?;
    /// ```
    pub async fn download(path: impl AsRef<std::path::Path>, url: &str) -> std::io::Result<()> {
        let response = reqwest::get(url).await
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?;
        let bytes = response.bytes().await
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?;
        tokio::fs::write(path, &bytes).await
    }
}

// ============================================================================
// 测试
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cosine_similarity() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![0.0, 1.0, 0.0];
        let sim = FaceEngine::cosine_similarity(&a, &b);
        assert!((sim - 0.0).abs() < 1e-6);

        let c = vec![1.0, 0.0, 0.0];
        let sim = FaceEngine::cosine_similarity(&a, &c);
        assert!((sim - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_euclidean_distance() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![4.0, 0.0, 0.0];
        let dist = FaceEngine::euclidean_distance(&a, &b);
        assert!((dist - 3.0).abs() < 1e-6);
    }
}