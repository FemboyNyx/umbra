import 'package:flutter/material.dart';

import '../theme/umbra_theme.dart';

/// Crescent mark used where the bitmap would be heavy or fail to load.
class UmbraMark extends StatelessWidget {
  const UmbraMark({super.key, this.size = 96});

  final double size;

  @override
  Widget build(BuildContext context) {
    return CustomPaint(
      size: Size.square(size),
      painter: const _CrescentPainter(),
    );
  }
}

class BrandLockup extends StatelessWidget {
  const BrandLockup({super.key, this.size = 120});

  final double size;

  @override
  Widget build(BuildContext context) {
    return ClipRRect(
      borderRadius: BorderRadius.circular(size * 0.28),
      child: Image.asset(
        'assets/branding/umbra-icon-concept.jpg',
        width: size,
        height: size,
        fit: BoxFit.cover,
        errorBuilder: (context, error, stackTrace) => UmbraMark(size: size),
      ),
    );
  }
}

class _CrescentPainter extends CustomPainter {
  const _CrescentPainter();

  @override
  void paint(Canvas canvas, Size size) {
    final rect = Offset.zero & size;
    canvas.drawRRect(
      RRect.fromRectAndRadius(rect, Radius.circular(size.width * 0.28)),
      Paint()..color = UmbraColors.background,
    );
    final center = size.center(Offset.zero);
    final radius = size.width * 0.32;
    final moon = Paint()..color = UmbraColors.silver;
    canvas.drawCircle(center, radius, moon);
    canvas.drawCircle(
      center.translate(size.width * 0.12, -size.width * 0.02),
      radius * 0.86,
      Paint()..color = UmbraColors.background,
    );
    final eye = Paint()..color = UmbraColors.violet;
    canvas.drawCircle(
      center.translate(-size.width * 0.08, -size.width * 0.02),
      size.width * 0.035,
      eye,
    );
  }

  @override
  bool shouldRepaint(covariant CustomPainter oldDelegate) => false;
}
