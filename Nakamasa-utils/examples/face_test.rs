//! 人脸识别对比测试 — 检测两张图片中的人脸并计算相似度
use std::path::Path;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let _env = ort::init();

    let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    let model_dir = root.join("models");

    let mut engine = nakamasa_utils::face::FaceEngine::new(
        model_dir.join("yunet.onnx"),
        model_dir.join("face_recognition_sface_2021dec.onnx"),
    )?;
    println!("✅ 引擎加载成功");

    // 比较两张图片
    let img1_path = root.join("IMG_20260719_183226.jpg");
    let img2_path = root.join("IMG_20260719_201858.jpg");

    let img1_bytes = std::fs::read(&img1_path)?;
    let img2_bytes = std::fs::read(&img2_path)?;

    println!("\n=== 图片1: {} ===", img1_path.file_name().unwrap().to_string_lossy());
    let r1 = engine.recognize(&img1_bytes)?;
    println!("检测到 {} 张人脸", r1.len());
    for (i, r) in r1.iter().enumerate() {
        println!("  [{i}] ({:.0},{:.0}) {:.0}x{:.0} conf={:.3} emb={}",
            r.face.x, r.face.y, r.face.w, r.face.h, r.face.confidence, r.embedding.len());
    }

    println!("\n=== 图片2: {} ===", img2_path.file_name().unwrap().to_string_lossy());
    let r2 = engine.recognize(&img2_bytes)?;
    println!("检测到 {} 张人脸", r2.len());
    for (i, r) in r2.iter().enumerate() {
        println!("  [{i}] ({:.0},{:.0}) {:.0}x{:.0} conf={:.3} emb={}",
            r.face.x, r.face.y, r.face.w, r.face.h, r.face.confidence, r.embedding.len());
    }

    // 交叉对比所有检测到的人脸
    println!("\n=== 人脸相似度对比 ===");
    for (i, a) in r1.iter().enumerate() {
        for (j, b) in r2.iter().enumerate() {
            let sim = nakamasa_utils::face::FaceEngine::cosine_similarity(&a.embedding, &b.embedding);
            let dist = nakamasa_utils::face::FaceEngine::euclidean_distance(&a.embedding, &b.embedding);
            println!("  图1[{i}] ↔ 图2[{j}]  sim={:.4}  dist={:.2}", sim, dist);
        }
    }

    Ok(())
}